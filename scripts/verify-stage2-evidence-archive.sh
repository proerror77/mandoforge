#!/usr/bin/env bash
set -euo pipefail

archive_path="${1:-}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

sha256_value() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    echo "sha256sum or shasum is required to verify the Stage 2 evidence archive" >&2
    exit 1
  fi
}

is_multi_tenant_target_kind() {
  case "$1" in
    multi_tenant_deployment|enterprise_multi_tenant|production_multi_tenant)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_real_cluster_kind() {
  case "$1" in
    k8s_cluster|kubernetes_cluster|production_cluster|real_cluster)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_distributed_state_backend() {
  case "$1" in
    juicefs|cephfs|longhorn-rwx)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_policy_controller_kind() {
  case "$1" in
    production_policy_controller|enterprise_policy_controller|external_policy_controller|policy_controller_cluster)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_environment() {
  case "$1" in
    production|prod)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_rollout_scope() {
  case "$1" in
    production|global|enterprise|multi_tenant)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

normalize_kind() {
  printf '%s' "$1" | tr '[:upper:]-' '[:lower:]_'
}

is_production_kms_backend_kind() {
  case "$(normalize_kind "$1")" in
    external_kms|aws_kms|gcp_kms|azure_key_vault|hashicorp_vault_transit|vault_transit|hsm|cloudhsm|pkcs11_hsm)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_accounting_or_erp_delivery_mode() {
  case "$1" in
    accounting*|erp*|netsuite|quickbooks|xero|sap|oracle_erp)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_production_identity_value() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

is_finance_system_identity_value() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  is_production_identity_value "$value" || return 1
  [[ ! "$value" =~ (^|[./:_-])(feishu|lark|drive|file|artifact)([./:_-]|$) ]] || return 1
}

artifact_issue() {
  local root="$1"
  local relative_path="$2"
  local path="$root/$relative_path"

  if [[ ! -s "$path" ]]; then
    printf '%s missing' "$relative_path"
    return 0
  fi

  case "$relative_path" in
    worker-load-validation-evidence.json)
      local evidence_status
      local controller_status
      local target_kind
      local node_count
      local cluster_id
      local load_validated
      local isolated_worker_pool_configured
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.controller_execution.node_count // 0' "$path")"
      cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$path")"
      load_validated="$(jq -r '.response.controller_execution.load_validated // false' "$path")"
      isolated_worker_pool_configured="$(jq -r '.response.controller_execution.isolated_worker_pool_configured // false' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$controller_status" != "validated" ]]; then
        printf '%s controller_status=%s' "$relative_path" "$controller_status"
        return 0
      fi
      if ! is_real_cluster_kind "$target_kind"; then
        printf '%s target_kind=%s is not a real cluster' "$relative_path" "$target_kind"
        return 0
      fi
      if [[ ! "$node_count" =~ ^[0-9]+$ || "$node_count" -lt 2 ]]; then
        printf '%s node_count=%s is not multi-node' "$relative_path" "$node_count"
        return 0
      fi
      if [[ -z "$cluster_id" ]]; then
        printf '%s cluster_id is missing' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$cluster_id"; then
        printf '%s cluster_id=%s is a pilot/mock/local identity' "$relative_path" "$cluster_id"
        return 0
      fi
      if [[ "$load_validated" != "true" ]]; then
        printf '%s load_validated=%s' "$relative_path" "$load_validated"
        return 0
      fi
      if [[ "$isolated_worker_pool_configured" != "true" ]]; then
        printf '%s isolated_worker_pool_configured=%s' "$relative_path" "$isolated_worker_pool_configured"
        return 0
      fi
      ;;
    remote-computer-state-sync-evidence.json)
      local evidence_status
      local controller_status
      local target_kind
      local node_count
      local state_backend
      local cluster_id
      local state_claim
      local checked_path_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.controller_execution.node_count // 0' "$path")"
      state_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // "unknown"' "$path")"
      cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$path")"
      state_claim="$(jq -r '.response.controller_execution.state_claim // ""' "$path")"
      checked_path_count="$(jq -r '.response.controller_execution.checked_path_count // 0' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$controller_status" != "validated" ]]; then
        printf '%s controller_status=%s' "$relative_path" "$controller_status"
        return 0
      fi
      if ! is_real_cluster_kind "$target_kind"; then
        printf '%s target_kind=%s is not a real cluster' "$relative_path" "$target_kind"
        return 0
      fi
      if [[ ! "$node_count" =~ ^[0-9]+$ || "$node_count" -lt 2 ]]; then
        printf '%s node_count=%s is not multi-node' "$relative_path" "$node_count"
        return 0
      fi
      if ! is_distributed_state_backend "$state_backend"; then
        printf '%s state_backend=%s is not distributed' "$relative_path" "$state_backend"
        return 0
      fi
      if [[ -z "$cluster_id" ]]; then
        printf '%s cluster_id is missing' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$cluster_id"; then
        printf '%s cluster_id=%s is a pilot/mock/local identity' "$relative_path" "$cluster_id"
        return 0
      fi
      if [[ -z "$state_claim" ]]; then
        printf '%s state_claim is missing' "$relative_path"
        return 0
      fi
      if [[ ! "$checked_path_count" =~ ^[0-9]+$ || "$checked_path_count" == "0" ]]; then
        printf '%s checked_path_count=%s' "$relative_path" "$checked_path_count"
        return 0
      fi
      ;;
    remote-computer-sidecar-recovery-evidence.json)
      local evidence_status
      local status
      local target_kind
      local node_count
      local replacement_scope
      local cluster_id
      local replacement_pods_healthy
      local checked_pod_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      status="$(jq -r '.response.validation_result.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.validation_result.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.validation_result.node_count // 0' "$path")"
      replacement_scope="$(jq -r '.response.validation_result.replacement_scope // "unknown"' "$path")"
      cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$path")"
      replacement_pods_healthy="$(jq -r '.response.validation_result.replacement_pods_healthy // false' "$path")"
      checked_pod_count="$(jq -r '.response.validation_result.checked_pod_count // 0' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$status" != "validated" ]]; then
        printf '%s validation_status=%s' "$relative_path" "$status"
        return 0
      fi
      if ! is_real_cluster_kind "$target_kind"; then
        printf '%s target_kind=%s is not a real cluster' "$relative_path" "$target_kind"
        return 0
      fi
      if [[ ! "$node_count" =~ ^[0-9]+$ || "$node_count" -lt 2 ]]; then
        printf '%s node_count=%s is not multi-node' "$relative_path" "$node_count"
        return 0
      fi
      if [[ "$replacement_scope" != "cluster" ]]; then
        printf '%s replacement_scope=%s is not cluster' "$relative_path" "$replacement_scope"
        return 0
      fi
      if [[ -z "$cluster_id" ]]; then
        printf '%s cluster_id is missing' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$cluster_id"; then
        printf '%s cluster_id=%s is a pilot/mock/local identity' "$relative_path" "$cluster_id"
        return 0
      fi
      if [[ "$replacement_pods_healthy" != "true" ]]; then
        printf '%s replacement_pods_healthy=%s' "$relative_path" "$replacement_pods_healthy"
        return 0
      fi
      if [[ ! "$checked_pod_count" =~ ^[0-9]+$ || "$checked_pod_count" == "0" ]]; then
        printf '%s checked_pod_count=%s' "$relative_path" "$checked_pod_count"
        return 0
      fi
      ;;
    worker-remote-computer/summary.json)
      local summary_status
      local production_blocked
      local same_cluster_target
      local worker_cluster_id
      local state_cluster_id
      local sidecar_cluster_id
      local state_backend
      local state_claim
      local state_checked_path_count
      local sidecar_replacement_pods_healthy
      local sidecar_checked_pod_count
      summary_status="$(jq -r '.status // "unknown"' "$path")"
      production_blocked="$(jq -r 'if has("production_blocked") then .production_blocked else true end' "$path")"
      same_cluster_target="$(jq -r '.same_cluster_target // false' "$path")"
      worker_cluster_id="$(jq -r '.worker.cluster_id // ""' "$path")"
      state_cluster_id="$(jq -r '.remote_computer.state_sync_cluster_id // ""' "$path")"
      sidecar_cluster_id="$(jq -r '.remote_computer.sidecar_cluster_id // ""' "$path")"
      state_backend="$(jq -r '.remote_computer.distributed_state_backend // "unknown"' "$path")"
      state_claim="$(jq -r '.remote_computer.state_claim // ""' "$path")"
      state_checked_path_count="$(jq -r '.remote_computer.checked_path_count // 0' "$path")"
      sidecar_replacement_pods_healthy="$(jq -r '.remote_computer.replacement_pods_healthy // false' "$path")"
      sidecar_checked_pod_count="$(jq -r '.remote_computer.checked_pod_count // 0' "$path")"
      if [[ "$summary_status" != "ready" || "$production_blocked" != "false" ]]; then
        printf '%s status=%s production_blocked=%s' "$relative_path" "$summary_status" "$production_blocked"
        return 0
      fi
      if [[ "$same_cluster_target" != "true" ]]; then
        printf '%s does not prove one shared cluster target' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$worker_cluster_id" || ! is_production_identity_value "$state_cluster_id" || ! is_production_identity_value "$sidecar_cluster_id"; then
        printf '%s contains a pilot/mock/local cluster id' "$relative_path"
        return 0
      fi
      if ! is_distributed_state_backend "$state_backend"; then
        printf '%s state_backend=%s is not distributed' "$relative_path" "$state_backend"
        return 0
      fi
      if [[ -z "$state_claim" ]]; then
        printf '%s state_claim is missing' "$relative_path"
        return 0
      fi
      if [[ ! "$state_checked_path_count" =~ ^[0-9]+$ || "$state_checked_path_count" == "0" ]]; then
        printf '%s checked_path_count=%s' "$relative_path" "$state_checked_path_count"
        return 0
      fi
      if [[ "$sidecar_replacement_pods_healthy" != "true" ]]; then
        printf '%s replacement_pods_healthy=%s' "$relative_path" "$sidecar_replacement_pods_healthy"
        return 0
      fi
      if [[ ! "$sidecar_checked_pod_count" =~ ^[0-9]+$ || "$sidecar_checked_pod_count" == "0" ]]; then
        printf '%s sidecar_checked_pod_count=%s' "$relative_path" "$sidecar_checked_pod_count"
        return 0
      fi
      ;;
    tenant-routing-validation-evidence.json)
      local evidence_status
      local validation_status
      local target_kind
      local tenant_count
      local tenant_sample_count
      local rls_enforced
      local rls_table_count
      local rls_forced_table_count
      local tenant_context_validated
      local cross_tenant_negative_tests
      local cross_tenant_negative_test_count
      local deployment_id
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      validation_status="$(jq -r '.response.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      tenant_count="$(jq -r '.response.controller_execution.tenant_count // 0' "$path")"
      tenant_sample_count="$(jq -r 'if ((.response.controller_execution.tenant_samples // null) | type) == "array" then (.response.controller_execution.tenant_samples | length) elif ((.response.controller_execution.tenant_ids_sample // null) | type) == "array" then (.response.controller_execution.tenant_ids_sample | length) else 0 end' "$path")"
      rls_enforced="$(jq -r '.response.controller_execution.rls_enforced // false' "$path")"
      rls_table_count="$(jq -r '.response.controller_execution.rls_table_count // .response.controller_execution.rls_enabled_table_count // 0' "$path")"
      rls_forced_table_count="$(jq -r '.response.controller_execution.rls_forced_table_count // .response.controller_execution.forced_rls_table_count // 0' "$path")"
      tenant_context_validated="$(jq -r '.response.controller_execution.tenant_context_validated // false' "$path")"
      cross_tenant_negative_tests="$(jq -r '.response.controller_execution.cross_tenant_negative_tests // false' "$path")"
      cross_tenant_negative_test_count="$(jq -r '.response.controller_execution.cross_tenant_negative_test_count // .response.controller_execution.negative_test_count // 0' "$path")"
      deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$validation_status" != "validated" ]]; then
        printf '%s validation_status=%s' "$relative_path" "$validation_status"
        return 0
      fi
      if ! is_multi_tenant_target_kind "$target_kind"; then
        printf '%s target_kind=%s is not broader multi-tenant' "$relative_path" "$target_kind"
        return 0
      fi
      if [[ ! "$tenant_count" =~ ^[0-9]+$ || "$tenant_count" -lt 2 ]]; then
        printf '%s tenant_count=%s is not multi-tenant' "$relative_path" "$tenant_count"
        return 0
      fi
      if [[ ! "$tenant_sample_count" =~ ^[0-9]+$ || "$tenant_sample_count" -lt 2 ]]; then
        printf '%s tenant_sample_count=%s is not an audited multi-tenant sample' "$relative_path" "$tenant_sample_count"
        return 0
      fi
      if [[ "$rls_enforced" != "true" || "$tenant_context_validated" != "true" || "$cross_tenant_negative_tests" != "true" ]]; then
        printf '%s tenant/RLS negative-test evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ ! "$rls_table_count" =~ ^[0-9]+$ || "$rls_table_count" == "0" ]]; then
        printf '%s rls_table_count=%s' "$relative_path" "$rls_table_count"
        return 0
      fi
      if [[ ! "$rls_forced_table_count" =~ ^[0-9]+$ || ! "$rls_table_count" =~ ^[0-9]+$ || "$rls_forced_table_count" -lt "$rls_table_count" ]]; then
        printf '%s rls_forced_table_count=%s is less than rls_table_count=%s' "$relative_path" "$rls_forced_table_count" "$rls_table_count"
        return 0
      fi
      if [[ ! "$cross_tenant_negative_test_count" =~ ^[0-9]+$ || "$cross_tenant_negative_test_count" == "0" ]]; then
        printf '%s cross_tenant_negative_test_count=%s' "$relative_path" "$cross_tenant_negative_test_count"
        return 0
      fi
      if ! is_production_identity_value "$deployment_id"; then
        printf '%s deployment_id=%s is a pilot/mock/local identity' "$relative_path" "${deployment_id:-<empty>}"
        return 0
      fi
      ;;
    policy-rollout-orchestration-validation-evidence.json)
      local evidence_status
      local status
      local controller_status
      local target_kind
      local environment
      local controller_id
      local rollout_scope
      local production_policy_store
      local rollback_supported
      local policy_store_id
      local deployment_id
      local step_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      status="$(jq -r '.response.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$path")"
      controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$path")"
      rollout_scope="$(jq -r '.response.controller_execution.rollout_scope // "unknown"' "$path")"
      production_policy_store="$(jq -r '.response.controller_execution.production_policy_store // false' "$path")"
      rollback_supported="$(jq -r '.response.controller_execution.rollback_supported // false' "$path")"
      policy_store_id="$(jq -r '.response.controller_execution.policy_store_id // ""' "$path")"
      deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$path")"
      step_count="$(jq -r 'if ((.response.controller_execution.steps // null) | type) == "array" then (.response.controller_execution.steps | length) else 0 end' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$status" != "validated" || "$controller_status" != "validated" ]]; then
        printf '%s status=%s controller_status=%s' "$relative_path" "$status" "$controller_status"
        return 0
      fi
      if ! is_production_policy_controller_kind "$target_kind"; then
        printf '%s target_kind=%s is not production policy controller' "$relative_path" "$target_kind"
        return 0
      fi
      if ! is_production_environment "$environment"; then
        printf '%s environment=%s is not production' "$relative_path" "$environment"
        return 0
      fi
      if [[ -z "$controller_id" ]]; then
        printf '%s controller_id is missing' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$controller_id"; then
        printf '%s controller_id=%s is a pilot/mock/local identity' "$relative_path" "$controller_id"
        return 0
      fi
      if ! is_production_rollout_scope "$rollout_scope"; then
        printf '%s rollout_scope=%s is not production-grade' "$relative_path" "$rollout_scope"
        return 0
      fi
      if [[ "$production_policy_store" != "true" || "$rollback_supported" != "true" ]]; then
        printf '%s policy store or rollback evidence incomplete' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$policy_store_id"; then
        printf '%s policy_store_id=%s is a pilot/mock/local identity' "$relative_path" "${policy_store_id:-<empty>}"
        return 0
      fi
      if ! is_production_identity_value "$deployment_id"; then
        printf '%s deployment_id=%s is a pilot/mock/local identity' "$relative_path" "${deployment_id:-<empty>}"
        return 0
      fi
      if [[ ! "$step_count" =~ ^[0-9]+$ || "$step_count" == "0" ]]; then
        printf '%s step_count=%s' "$relative_path" "$step_count"
        return 0
      fi
      ;;
    policy-rollout-due-run-evidence.json)
      local evidence_status
      local due_run_status
      local scanned_count
      local checked_at
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      due_run_status="$(jq -r '.response.status // "unknown"' "$path")"
      scanned_count="$(jq -r '.response.scanned_count // 0' "$path")"
      checked_at="$(jq -r '.response.checked_at // ""' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$due_run_status" != "activated" && "$due_run_status" != "noop" ]]; then
        printf '%s due_run_status=%s' "$relative_path" "$due_run_status"
        return 0
      fi
      if [[ ! "$scanned_count" =~ ^[0-9]+$ || "$scanned_count" == "0" ]]; then
        printf '%s scanned_count=%s' "$relative_path" "$scanned_count"
        return 0
      fi
      if [[ -z "$checked_at" ]]; then
        printf '%s checked_at is missing' "$relative_path"
        return 0
      fi
      ;;
    vault-kms-rotation-evidence.json)
      local evidence_status
      local status
      local execution_status
      local production_backend
      local backend_kind
      local environment
      local backend_id
      local key_id
      local rotation_id
      local rotated_count
      local catalog_updated_count
      local action_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      status="$(jq -r '.response.status // "unknown"' "$path")"
      execution_status="$(jq -r '.response.external_execution.status // "unknown"' "$path")"
      production_backend="$(jq -r '.response.external_execution.production_backend // false' "$path")"
      backend_kind="$(jq -r '.response.external_execution.backend_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.external_execution.environment // "unknown"' "$path")"
      backend_id="$(jq -r '.response.external_execution.backend_id // ""' "$path")"
      key_id="$(jq -r '.response.external_execution.key_id // ""' "$path")"
      rotation_id="$(jq -r '.response.external_execution.rotation_id // ""' "$path")"
      rotated_count="$(jq -r '.response.rotated_count // .response.external_execution.rotated_count // 0' "$path")"
      catalog_updated_count="$(jq -r '.response.catalog_updated_count // 0' "$path")"
      action_count="$(jq -r 'if ((.response.actions // null) | type) == "array" then (.response.actions | length) else 0 end' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$status" != "validated" || "$execution_status" != "validated" || "$production_backend" != "true" ]]; then
        printf '%s status=%s execution_status=%s production_backend=%s' "$relative_path" "$status" "$execution_status" "$production_backend"
        return 0
      fi
      if ! is_production_kms_backend_kind "$backend_kind"; then
        printf '%s backend_kind=%s is not production KMS/HSM' "$relative_path" "$backend_kind"
        return 0
      fi
      if ! is_production_environment "$environment"; then
        printf '%s environment=%s is not production' "$relative_path" "$environment"
        return 0
      fi
      if [[ -z "$backend_id" || -z "$key_id" ]]; then
        printf '%s backend_id or key_id is missing' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$backend_id"; then
        printf '%s backend_id=%s is a pilot/mock/local identity' "$relative_path" "$backend_id"
        return 0
      fi
      if ! is_production_identity_value "$key_id"; then
        printf '%s key_id=%s is a pilot/mock/local identity' "$relative_path" "$key_id"
        return 0
      fi
      if ! is_production_identity_value "$rotation_id"; then
        printf '%s rotation_id=%s is a pilot/mock/local identity' "$relative_path" "${rotation_id:-<empty>}"
        return 0
      fi
      if [[ ! "$rotated_count" =~ ^[0-9]+$ || "$rotated_count" == "0" ]]; then
        printf '%s rotated_count=%s' "$relative_path" "$rotated_count"
        return 0
      fi
      if [[ ! "$catalog_updated_count" =~ ^[0-9]+$ || "$catalog_updated_count" == "0" ]]; then
        printf '%s catalog_updated_count=%s' "$relative_path" "$catalog_updated_count"
        return 0
      fi
      if [[ ! "$action_count" =~ ^[0-9]+$ || "$action_count" == "0" ]]; then
        printf '%s action_count=%s' "$relative_path" "$action_count"
        return 0
      fi
      ;;
    vault-kms-recovery-evidence.json)
      local evidence_status
      local status
      local controller_status
      local backend_kind
      local environment
      local backend_id
      local key_id
      local recovery_id
      local recovery_target_kind
      local step_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      status="$(jq -r '.response.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      backend_kind="$(jq -r '.response.controller_execution.backend_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$path")"
      backend_id="$(jq -r '.response.controller_execution.backend_id // ""' "$path")"
      key_id="$(jq -r '.response.controller_execution.key_id // ""' "$path")"
      recovery_id="$(jq -r '.response.controller_execution.recovery_id // ""' "$path")"
      recovery_target_kind="$(jq -r '.response.controller_execution.recovery_target_kind // "unknown"' "$path")"
      step_count="$(jq -r 'if ((.response.controller_execution.steps // null) | type) == "array" then (.response.controller_execution.steps | length) else 0 end' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$status" != "validated" || "$controller_status" != "validated" ]]; then
        printf '%s status=%s controller_status=%s' "$relative_path" "$status" "$controller_status"
        return 0
      fi
      if ! is_production_kms_backend_kind "$backend_kind"; then
        printf '%s backend_kind=%s is not production KMS/HSM' "$relative_path" "$backend_kind"
        return 0
      fi
      if ! is_production_environment "$environment"; then
        printf '%s environment=%s is not production' "$relative_path" "$environment"
        return 0
      fi
      if [[ -z "$backend_id" || -z "$key_id" ]]; then
        printf '%s backend_id or key_id is missing' "$relative_path"
        return 0
      fi
      if ! is_production_identity_value "$backend_id"; then
        printf '%s backend_id=%s is a pilot/mock/local identity' "$relative_path" "$backend_id"
        return 0
      fi
      if ! is_production_identity_value "$key_id"; then
        printf '%s key_id=%s is a pilot/mock/local identity' "$relative_path" "$key_id"
        return 0
      fi
      if ! is_production_identity_value "$recovery_id"; then
        printf '%s recovery_id=%s is a pilot/mock/local identity' "$relative_path" "${recovery_id:-<empty>}"
        return 0
      fi
      if [[ "$recovery_target_kind" != "production_kms_backend" && "$recovery_target_kind" != "production_hsm_backend" && "$recovery_target_kind" != "enterprise_kms_backend" ]]; then
        printf '%s recovery_target_kind=%s is not production' "$relative_path" "$recovery_target_kind"
        return 0
      fi
      if [[ ! "$step_count" =~ ^[0-9]+$ || "$step_count" == "0" ]]; then
        printf '%s step_count=%s' "$relative_path" "$step_count"
        return 0
      fi
      ;;
    finance-export-delivery-observer.json)
      local status
      local delivery_mode
      local delivery_count
      local system_id
      status="$(jq -r '.status // "unknown"' "$path")"
      delivery_mode="$(jq -r '.export_state.delivery_mode // "unknown"' "$path")"
      delivery_count="$(jq -r '.export_state.delivery_count // 0' "$path")"
      system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$path")"
      if [[ "$status" != "ok" ]]; then
        printf '%s observer_status=%s' "$relative_path" "$status"
        return 0
      fi
      if [[ ! "$delivery_count" =~ ^[0-9]+$ || "$delivery_count" == "0" ]]; then
        printf '%s delivery_count=%s' "$relative_path" "$delivery_count"
        return 0
      fi
      if ! is_accounting_or_erp_delivery_mode "$delivery_mode"; then
        printf '%s delivery_mode=%s is not accounting/ERP' "$relative_path" "$delivery_mode"
        return 0
      fi
      if [[ -z "$system_id" ]]; then
        printf '%s system_id is missing' "$relative_path"
        return 0
      fi
      if ! is_finance_system_identity_value "$system_id"; then
        printf '%s system_id=%s is not a true ERP/accounting system identity' "$relative_path" "$system_id"
        return 0
      fi
      ;;
    finance-close-evidence.json)
      local evidence_status
      local run_status
      local close_configured
      local close_status
      local close_id
      local step_count
      local action_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      run_status="$(jq -r '.response.status // "unknown"' "$path")"
      close_configured="$(jq -r '.response.close_controller_configured // false' "$path")"
      close_status="$(jq -r '.response.close_controller_execution.status // "unknown"' "$path")"
      close_id="$(jq -r '.response.close_controller_execution.close_id // ""' "$path")"
      step_count="$(jq -r 'if ((.response.close_controller_execution.steps // null) | type) == "array" then (.response.close_controller_execution.steps | length) else 0 end' "$path")"
      action_count="$(jq -r '[.response.actions[]? | select(. == "usage_finance_close_controller_executed")] | length' "$path")"
      if [[ "$evidence_status" != "captured" || "$run_status" != "completed" ]]; then
        printf '%s evidence_status=%s run_status=%s' "$relative_path" "$evidence_status" "$run_status"
        return 0
      fi
      if [[ "$close_configured" != "true" || "$close_status" != "closed" ]]; then
        printf '%s close_configured=%s close_status=%s' "$relative_path" "$close_configured" "$close_status"
        return 0
      fi
      if ! is_finance_system_identity_value "$close_id"; then
        printf '%s close_id=%s is not a true ERP/accounting system identity' "$relative_path" "${close_id:-<empty>}"
        return 0
      fi
      if [[ ! "$step_count" =~ ^[0-9]+$ || "$step_count" == "0" ]]; then
        printf '%s step_count=%s' "$relative_path" "$step_count"
        return 0
      fi
      if [[ ! "$action_count" =~ ^[0-9]+$ || "$action_count" == "0" ]]; then
        printf '%s close controller action missing' "$relative_path"
        return 0
      fi
      ;;
    finance-reconciliation-evidence.json)
      local evidence_status
      local reconciliation_status
      local reconciliation_id
      local check_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      reconciliation_status="$(jq -r '.response.status // "unknown"' "$path")"
      reconciliation_id="$(jq -r '.response.reconciliation_id // ""' "$path")"
      check_count="$(jq -r 'if ((.response.checks // null) | type) == "array" then (.response.checks | length) else 0 end' "$path")"
      if [[ "$evidence_status" != "captured" || "$reconciliation_status" != "reconciled" ]]; then
        printf '%s evidence_status=%s reconciliation_status=%s' "$relative_path" "$evidence_status" "$reconciliation_status"
        return 0
      fi
      if ! is_finance_system_identity_value "$reconciliation_id"; then
        printf '%s reconciliation_id=%s is not a true ERP/accounting system identity' "$relative_path" "${reconciliation_id:-<empty>}"
        return 0
      fi
      if [[ ! "$check_count" =~ ^[0-9]+$ || "$check_count" == "0" ]]; then
        printf '%s check_count=%s' "$relative_path" "$check_count"
        return 0
      fi
      ;;
    usage-export-csv-evidence.json)
      local http_status
      local byte_count
      http_status="$(jq -r '.http_status // 0' "$path")"
      byte_count="$(jq -r '.byte_count // 0' "$path")"
      if [[ ! "$http_status" =~ ^2[0-9][0-9]$ ]]; then
        printf '%s http_status=%s' "$relative_path" "$http_status"
        return 0
      fi
      if [[ ! "$byte_count" =~ ^[0-9]+$ || "$byte_count" == "0" ]]; then
        printf '%s byte_count=%s' "$relative_path" "$byte_count"
        return 0
      fi
      ;;
    finance-export-delivery-evidence.json)
      local evidence_status
      local delivery_status
      local delivered
      local target_configured
      local byte_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      delivery_status="$(jq -r '.response.status // "unknown"' "$path")"
      delivered="$(jq -r '.response.delivered // false' "$path")"
      target_configured="$(jq -r '.response.target_configured // false' "$path")"
      byte_count="$(jq -r '.response.bytes // 0' "$path")"
      if [[ "$evidence_status" != "captured" || "$delivery_status" != "delivered" || "$delivered" != "true" ]]; then
        printf '%s evidence_status=%s delivery_status=%s delivered=%s' "$relative_path" "$evidence_status" "$delivery_status" "$delivered"
        return 0
      fi
      if [[ "$target_configured" != "true" ]]; then
        printf '%s target_configured=%s' "$relative_path" "$target_configured"
        return 0
      fi
      if [[ ! "$byte_count" =~ ^[0-9]+$ || "$byte_count" == "0" ]]; then
        printf '%s byte_count=%s' "$relative_path" "$byte_count"
        return 0
      fi
      ;;
    managed-session-restart-resume-evidence.json)
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
      status="$(jq -r '.status // "unknown"' "$path")"
      target_id="$(jq -r '.target.id // .target.cluster_id // .target.deployment_id // ""' "$path")"
      target_kind="$(jq -r '.target.kind // "unknown"' "$path")"
      enqueue_event_persisted="$(jq -r '.session_loop.enqueue_event_persisted // false' "$path")"
      worker_drain_observed="$(jq -r '.session_loop.worker_drain_observed // false' "$path")"
      api_restarted="$(jq -r '.restart.api_restarted // false' "$path")"
      worker_restarted="$(jq -r '.restart.worker_restarted // false' "$path")"
      session_state_resumed="$(jq -r '.resume.session_state_resumed // false' "$path")"
      processed_event_seq_preserved="$(jq -r '.resume.processed_event_seq_preserved // false' "$path")"
      thread_lineage_preserved="$(jq -r '.thread_lineage.preserved // false' "$path")"
      finalization_fenced="$(jq -r '.lease_fencing.finalization_fenced // false' "$path")"
      stale_worker_rejected="$(jq -r '.lease_fencing.stale_worker_rejected // false' "$path")"
      runtime_turn_completed="$(jq -r '.runtime_turn.completed // false' "$path")"
      final_message_preserved="$(jq -r '.runtime_turn.final_message_preserved // false' "$path")"
      if ! [[ "$status" == "validated" || "$status" == "completed" || "$status" == "ready" ]]; then
        printf '%s status=%s' "$relative_path" "$status"
        return 0
      fi
      if ! is_production_identity_value "$target_id"; then
        printf '%s target_id=%s is a pilot/mock/local identity' "$relative_path" "${target_id:-<empty>}"
        return 0
      fi
      case "$target_kind" in
        managed_session_runtime|production_runtime_cluster|managed_agent_cluster)
          ;;
        *)
          printf '%s target_kind=%s is not managed-session production runtime' "$relative_path" "$target_kind"
          return 0
          ;;
      esac
      if [[ "$enqueue_event_persisted" != "true" || "$worker_drain_observed" != "true" ]]; then
        printf '%s session-loop enqueue/drain evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ "$api_restarted" != "true" || "$worker_restarted" != "true" ]]; then
        printf '%s API/worker restart evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ "$session_state_resumed" != "true" || "$processed_event_seq_preserved" != "true" ]]; then
        printf '%s session resume or processed cursor evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ "$thread_lineage_preserved" != "true" ]]; then
        printf '%s thread lineage evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ "$finalization_fenced" != "true" || "$stale_worker_rejected" != "true" ]]; then
        printf '%s lease fencing evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ "$runtime_turn_completed" != "true" || "$final_message_preserved" != "true" ]]; then
        printf '%s runtime turn finalization evidence incomplete' "$relative_path"
        return 0
      fi
      ;;
  esac
}

value_mismatch_issue() {
  local label="$1"
  local expected="$2"
  local actual="$3"

  if [[ -z "$expected" ]]; then
    printf '%s expected value is missing' "$label"
    return 0
  fi
  if [[ -z "$actual" ]]; then
    printf '%s actual value is missing' "$label"
    return 0
  fi
  if [[ "$expected" != "$actual" ]]; then
    printf '%s expected=%s actual=%s' "$label" "$expected" "$actual"
    return 0
  fi
  return 0
}

target_identity_issue() {
  local label="$1"
  local value="$2"

  if ! is_production_identity_value "$value"; then
    printf '%s=%s is a pilot/mock/local identity' "$label" "${value:-<empty>}"
    return 0
  fi
  return 0
}

finance_identity_issue() {
  local label="$1"
  local value="$2"

  if ! is_finance_system_identity_value "$value"; then
    printf '%s=%s is not a true ERP/accounting system identity' "$label" "${value:-<empty>}"
    return 0
  fi
  return 0
}

verify_run_manifest() {
  local root="$1"
  local manifest="$root/production-evidence-run.json"
  local issue_count=0
  local issue
  local expected_cluster_id
  local worker_cluster_id
  local state_cluster_id
  local sidecar_cluster_id
  local summary_worker_cluster_id
  local summary_state_cluster_id
  local summary_sidecar_cluster_id
  local expected_tenant_deployment_id
  local tenant_deployment_id
  local expected_policy_controller_id
  local policy_controller_id
  local expected_kms_backend_id
  local expected_kms_key_id
  local rotation_backend_id
  local rotation_key_id
  local recovery_backend_id
  local recovery_key_id
  local expected_finance_system_id
  local finance_system_id
  local expected_managed_session_runtime_target_id
  local managed_session_runtime_target_id

  if [[ ! -s "$manifest" ]]; then
    echo "Stage 2 evidence archive semantic issue: production-evidence-run.json missing" >&2
    return 1
  fi

  expected_cluster_id="$(jq -r '.expected_targets.worker_remote_computer.cluster_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected worker/Remote Computer cluster id" "$expected_cluster_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/worker-load-validation-evidence.json" 2>/dev/null || echo "")"
  state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$root/remote-computer-sidecar-recovery-evidence.json" 2>/dev/null || echo "")"
  summary_worker_cluster_id="$(jq -r '.worker.cluster_id // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_state_cluster_id="$(jq -r '.remote_computer.state_sync_cluster_id // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_sidecar_cluster_id="$(jq -r '.remote_computer.sidecar_cluster_id // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  for issue in \
    "$(value_mismatch_issue "worker cluster id" "$expected_cluster_id" "$worker_cluster_id")" \
    "$(value_mismatch_issue "remote state-sync cluster id" "$expected_cluster_id" "$state_cluster_id")" \
    "$(value_mismatch_issue "remote sidecar cluster id" "$expected_cluster_id" "$sidecar_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary worker cluster id" "$expected_cluster_id" "$summary_worker_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary state-sync cluster id" "$expected_cluster_id" "$summary_state_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary sidecar cluster id" "$expected_cluster_id" "$summary_sidecar_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary worker evidence cluster id" "$worker_cluster_id" "$summary_worker_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary state-sync evidence cluster id" "$state_cluster_id" "$summary_state_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary sidecar evidence cluster id" "$sidecar_cluster_id" "$summary_sidecar_cluster_id")"; do
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

  expected_tenant_deployment_id="$(jq -r '.expected_targets.tenant_routing.deployment_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected tenant routing deployment id" "$expected_tenant_deployment_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  tenant_deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$root/tenant-routing-validation-evidence.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "tenant routing deployment id" "$expected_tenant_deployment_id" "$tenant_deployment_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi

  expected_policy_controller_id="$(jq -r '.expected_targets.policy_rollout.controller_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected policy rollout controller id" "$expected_policy_controller_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  policy_controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$root/policy-rollout-orchestration-validation-evidence.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "policy rollout controller id" "$expected_policy_controller_id" "$policy_controller_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi

  expected_kms_backend_id="$(jq -r '.expected_targets.vault_kms.backend_id // ""' "$manifest")"
  expected_kms_key_id="$(jq -r '.expected_targets.vault_kms.key_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected KMS backend id" "$expected_kms_backend_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  rotation_backend_id="$(jq -r '.response.external_execution.backend_id // ""' "$root/vault-kms-rotation-evidence.json" 2>/dev/null || echo "")"
  rotation_key_id="$(jq -r '.response.external_execution.key_id // ""' "$root/vault-kms-rotation-evidence.json" 2>/dev/null || echo "")"
  recovery_backend_id="$(jq -r '.response.controller_execution.backend_id // ""' "$root/vault-kms-recovery-evidence.json" 2>/dev/null || echo "")"
  recovery_key_id="$(jq -r '.response.controller_execution.key_id // ""' "$root/vault-kms-recovery-evidence.json" 2>/dev/null || echo "")"
  for issue in \
    "$(value_mismatch_issue "KMS rotation backend id" "$expected_kms_backend_id" "$rotation_backend_id")" \
    "$(value_mismatch_issue "KMS rotation key id" "$expected_kms_key_id" "$rotation_key_id")" \
    "$(value_mismatch_issue "KMS recovery backend id" "$expected_kms_backend_id" "$recovery_backend_id")" \
    "$(value_mismatch_issue "KMS recovery key id" "$expected_kms_key_id" "$recovery_key_id")"; do
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

  expected_finance_system_id="$(jq -r '.expected_targets.finance.system_id // ""' "$manifest")"
  issue="$(finance_identity_issue "expected finance ERP system id" "$expected_finance_system_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  finance_system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$root/finance-export-delivery-observer.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "finance ERP system id" "$expected_finance_system_id" "$finance_system_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi

  expected_managed_session_runtime_target_id="$(jq -r '.expected_targets.managed_session_runtime.target_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected managed-session runtime target id" "$expected_managed_session_runtime_target_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  managed_session_runtime_target_id="$(jq -r '.target.id // .target.cluster_id // .target.deployment_id // ""' "$root/managed-session-restart-resume-evidence.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "managed-session runtime target id" "$expected_managed_session_runtime_target_id" "$managed_session_runtime_target_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi

  return "$issue_count"
}

verify_semantic_artifacts() {
  local root="$1"
  local issue_count=0
  local artifact
  local issue
  local worker_cluster_id
  local state_cluster_id
  local sidecar_cluster_id
  local required_artifacts=(
    production-evidence-run.json
    worker-load-validation-evidence.json
    remote-computer-state-sync-evidence.json
    remote-computer-sidecar-recovery-evidence.json
    worker-remote-computer/summary.json
    tenant-routing-validation-evidence.json
    policy-rollout-orchestration-validation-evidence.json
    policy-rollout-due-run-evidence.json
    vault-kms-rotation-evidence.json
    vault-kms-recovery-evidence.json
    finance-close-evidence.json
    finance-reconciliation-evidence.json
    usage-export-csv-evidence.json
    finance-export-delivery-evidence.json
    finance-export-delivery-observer.json
    managed-session-restart-resume-evidence.json
  )

  for artifact in "${required_artifacts[@]}"; do
    issue="$(artifact_issue "$root" "$artifact")"
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

  worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/worker-load-validation-evidence.json" 2>/dev/null || echo "")"
  state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$root/remote-computer-sidecar-recovery-evidence.json" 2>/dev/null || echo "")"
  if [[ -n "$worker_cluster_id" && -n "$state_cluster_id" && -n "$sidecar_cluster_id" ]]; then
    if ! [[ "$worker_cluster_id" == "$state_cluster_id" && "$worker_cluster_id" == "$sidecar_cluster_id" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: worker, state-sync, and sidecar evidence do not share one cluster id" >&2
    fi
  fi

  set +e
  verify_run_manifest "$root"
  issue=$?
  set -e
  issue_count=$((issue_count + issue))

  return "$issue_count"
}

verify_archive() {
  local archive="$1"
  local checksum_file="${archive}.sha256"
  local manifest_file="${archive}.manifest.txt"
  local tmpdir
  local checklist
  local expected_sha
  local actual_sha
  local manifest_sha
  local completion_blocked
  local missing_total
  local semantic_issue_count

  if [[ ! -s "$archive" ]]; then
    echo "missing or empty Stage 2 evidence archive: $archive" >&2
    exit 1
  fi

  if [[ ! -s "$checksum_file" ]]; then
    echo "missing Stage 2 evidence checksum sidecar: $checksum_file" >&2
    exit 1
  fi

  if [[ ! -s "$manifest_file" ]]; then
    echo "missing Stage 2 evidence release manifest: $manifest_file" >&2
    exit 1
  fi

  expected_sha="$(awk '{print $1}' "$checksum_file")"
  actual_sha="$(sha256_value "$archive")"

  if [[ "$expected_sha" != "$actual_sha" ]]; then
    echo "Stage 2 evidence archive checksum mismatch for $archive" >&2
    echo "expected=$expected_sha" >&2
    echo "actual=$actual_sha" >&2
    exit 1
  fi

  manifest_sha="$(grep -E '^archive_sha256=' "$manifest_file" | sed 's/^archive_sha256=//')"
  if [[ "$manifest_sha" != "$actual_sha" ]]; then
    echo "Stage 2 evidence archive manifest checksum mismatch for $archive" >&2
    echo "manifest=$manifest_sha" >&2
    echo "actual=$actual_sha" >&2
    exit 1
  fi

  if ! grep -q "^archive_path=$archive$" "$manifest_file"; then
    echo "Stage 2 evidence archive manifest does not point at $archive" >&2
    exit 1
  fi

  tar tzf "$archive" >/dev/null

  if ! command -v jq >/dev/null 2>&1; then
    echo "jq is required to verify the Stage 2 completion checklist inside the archive" >&2
    exit 1
  fi

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' RETURN
  tar xzf "$archive" -C "$tmpdir"
  checklist="$tmpdir/completion-audit/checklist.json"
  if [[ ! -s "$checklist" ]]; then
    echo "Stage 2 evidence archive is missing completion-audit/checklist.json" >&2
    exit 1
  fi

  completion_blocked="$(jq -r 'if has("completion_blocked") then .completion_blocked else true end' "$checklist")"
  missing_total="$(jq -r '
    [
      .missing_readiness_endpoint_count,
      .missing_validation_endpoint_count,
      .missing_required_evidence_artifact_count,
      .stale_readiness_artifact_count,
      .stale_validation_artifact_count,
      .stale_required_evidence_artifact_count,
      .unresolved_endpoint_count,
      .missing_evidence_script_count,
      .missing_evidence_job_manifest_count,
      .missing_required_flag_count
    ]
    | map(. // 0)
    | add
  ' "$checklist")"

  if [[ "$completion_blocked" == "true" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 evidence archive checklist is still blocked; set ALLOW_BLOCKED=1 only for inventory archives." >&2
    exit 1
  fi

  if [[ "$missing_total" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 evidence archive checklist still has missing or stale evidence metadata/artifacts: $missing_total" >&2
    exit 1
  fi

  set +e
  verify_semantic_artifacts "$tmpdir"
  semantic_issue_count="$?"
  set -e
  if [[ "$semantic_issue_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Stage 2 evidence archive contains non-production or incomplete semantic evidence: $semantic_issue_count" >&2
    exit 1
  fi
}

self_test() {
  local tmpdir
  local archive
  local sha

  tmpdir="$(mktemp -d)"
  mkdir -p "$tmpdir/evidence/completion-audit"
  echo "stage2_status=blocked" >"$tmpdir/evidence/summary.txt"
  cat >"$tmpdir/evidence/completion-audit/checklist.json" <<'JSON'
{
  "completion_blocked": false,
  "missing_readiness_endpoint_count": 0,
  "missing_validation_endpoint_count": 0,
  "missing_required_evidence_artifact_count": 0,
  "stale_readiness_artifact_count": 0,
  "stale_validation_artifact_count": 0,
  "stale_required_evidence_artifact_count": 0,
  "unresolved_endpoint_count": 0,
  "missing_evidence_script_count": 0,
  "missing_evidence_job_manifest_count": 0,
  "missing_required_flag_count": 0
}
JSON
  cat >"$tmpdir/evidence/worker-load-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "load_validated": true,
      "isolated_worker_pool_configured": true
    }
  }
}
JSON
  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1"
    },
    "vault_kms": {
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1"
    },
    "finance": {
      "system_id": "netsuite-prod-1"
    },
    "managed_session_runtime": {
      "target_id": "managed-session-runtime-prod-1"
    }
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "distributed_state_backend": "juicefs",
      "state_claim": "mandoforge-remote-computer-state",
      "checked_path_count": 6
    }
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "cluster",
      "replacement_pods_healthy": true,
      "checked_pod_count": 1
    }
  }
}
JSON
  mkdir -p "$tmpdir/evidence/worker-remote-computer"
  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1"
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "replacement_pods_healthy": true,
    "checked_pod_count": 1
  }
}
JSON
  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": ["tenant-a", "tenant-b"],
      "rls_enforced": true,
      "rls_table_count": 12,
      "rls_forced_table_count": 12,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 3
    }
  }
}
JSON
  cat >"$tmpdir/evidence/policy-rollout-orchestration-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "target_kind": "production_policy_controller",
      "environment": "production",
      "controller_id": "policy-controller-prod-1",
      "rollout_scope": "global",
      "production_policy_store": true,
      "rollback_supported": true,
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "status": "passed"},
        {"name": "staged-runtime-clear", "status": "passed"}
      ]
    }
  }
}
JSON
  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "scanned_count": 1,
    "skipped_count": 1,
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  cat >"$tmpdir/evidence/finance-export-delivery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "delivered",
    "delivered": true,
    "target_configured": false,
    "bytes": 128
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-target-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-target-negative.out 2>/tmp/mandoforge-stage2-archive-finance-target-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected unconfigured finance delivery target to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "delivered",
    "delivered": true,
    "target_configured": true,
    "bytes": 128
  }
}
JSON
  cat >"$tmpdir/evidence/vault-kms-rotation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "external_execution": {
      "status": "validated",
      "production_backend": true,
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "rotation_id": "kms-rotation-1",
      "rotated_count": 1
    },
    "rotated_count": 1,
    "catalog_updated_count": 1,
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  cat >"$tmpdir/evidence/vault-kms-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "recovery_id": "kms-recovery-1",
      "recovery_target_kind": "production_kms_backend",
      "steps": [
        {"name": "restore-key-material", "status": "validated"},
        {"name": "verify-secret-consumers", "status": "passed"}
      ]
    }
  }
}
JSON
  cat >"$tmpdir/evidence/finance-close-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "completed",
    "actions": ["usage_finance_close_controller_executed"],
    "close_controller_configured": true,
    "close_controller_execution": {
      "status": "closed",
      "close_id": "netsuite-close-prod-1",
      "steps": [
        {"name": "export-present", "status": "passed"},
        {"name": "accounting-period-open", "status": "passed"}
      ]
    }
  }
}
JSON
  cat >"$tmpdir/evidence/finance-reconciliation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "reconciled",
    "reconciliation_id": "netsuite-reconciliation-prod-1",
    "checks": [
      {"name": "close-evidence", "status": "passed"},
      {"name": "export-recent", "status": "passed"}
    ]
  }
}
JSON
  cat >"$tmpdir/evidence/usage-export-csv-evidence.json" <<'JSON'
{
  "http_status": 200,
  "byte_count": 128
}
JSON
  cat >"$tmpdir/evidence/finance-export-delivery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "delivered",
    "delivered": true,
    "target_configured": true,
    "bytes": 128
  }
}
JSON
  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "delivery_count": 1
  }
}
JSON
  cat >"$tmpdir/evidence/managed-session-restart-resume-evidence.json" <<'JSON'
{
  "status": "validated",
  "target": {
    "id": "managed-session-runtime-prod-1",
    "kind": "managed_session_runtime"
  },
  "session_loop": {
    "enqueue_event_persisted": true,
    "worker_drain_observed": true
  },
  "restart": {
    "api_restarted": true,
    "worker_restarted": true
  },
  "resume": {
    "session_state_resumed": true,
    "processed_event_seq_preserved": true
  },
  "thread_lineage": {
    "preserved": true
  },
  "lease_fencing": {
    "finalization_fenced": true,
    "stale_worker_rejected": true
  },
  "runtime_turn": {
    "completed": true,
    "final_message_preserved": true
  }
}
JSON
  archive="$tmpdir/stage2-evidence.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  verify_archive "$archive"

  cat >"$tmpdir/evidence/worker-load-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 1,
      "cluster_id": "prod-cluster-1",
      "load_validated": true,
      "isolated_worker_pool_configured": true
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-worker-single-node-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-worker-single-node-negative.out 2>/tmp/mandoforge-stage2-archive-worker-single-node-negative.err
  local negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected single-node worker evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-load-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "load_validated": true,
      "isolated_worker_pool_configured": false
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-worker-isolated-pool-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-worker-isolated-pool-negative.out 2>/tmp/mandoforge-stage2-archive-worker-isolated-pool-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing isolated worker pool evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-load-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "load_validated": false,
      "isolated_worker_pool_configured": true
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-worker-load-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-worker-load-negative.out 2>/tmp/mandoforge-stage2-archive-worker-load-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing worker load validation evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-load-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "load_validated": true,
      "isolated_worker_pool_configured": true
    }
  }
}
JSON

  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "distributed_state_backend": "hostpath",
      "state_claim": "mandoforge-remote-computer-state",
      "checked_path_count": 6
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-backend-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-backend-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-backend-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected local Remote Computer state backend evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "distributed_state_backend": "juicefs",
      "state_claim": "",
      "checked_path_count": 6
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-claim-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-claim-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-claim-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing Remote Computer state claim evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "distributed_state_backend": "juicefs",
      "state_claim": "mandoforge-remote-computer-state",
      "checked_path_count": 0
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-path-count-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-path-count-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-path-count-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero Remote Computer state path evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "distributed_state_backend": "juicefs",
      "state_claim": "mandoforge-remote-computer-state",
      "checked_path_count": 6
    }
  }
}
JSON

  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "cluster",
      "replacement_pods_healthy": true,
      "checked_pod_count": 0
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-sidecar-pod-count-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-sidecar-pod-count-negative.out 2>/tmp/mandoforge-stage2-archive-sidecar-pod-count-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero sidecar checked Pod count to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "cluster",
      "replacement_pods_healthy": false,
      "checked_pod_count": 1
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-sidecar-health-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-sidecar-health-negative.out 2>/tmp/mandoforge-stage2-archive-sidecar-health-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected unhealthy sidecar replacement Pod evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "pod",
      "replacement_pods_healthy": true,
      "checked_pod_count": 1
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-sidecar-scope-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-sidecar-scope-negative.out 2>/tmp/mandoforge-stage2-archive-sidecar-scope-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected pod-scoped sidecar replacement evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "cluster",
      "replacement_pods_healthy": true,
      "checked_pod_count": 1
    }
  }
}
JSON

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "lark_drive",
    "system_id": "lark-drive-whiskey",
    "delivery_count": 1
  }
}
JSON
  archive="$tmpdir/stage2-evidence-pilot.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-negative.out 2>/tmp/mandoforge-stage2-archive-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected non-ERP finance evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "delivery_count": 0
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-zero-delivery.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-zero-delivery.out 2>/tmp/mandoforge-stage2-archive-finance-zero-delivery.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero finance delivery count to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "delivery_count": 1
  }
}
JSON
  cat >"$tmpdir/evidence/vault-kms-rotation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "external_execution": {
      "status": "validated",
      "production_backend": true,
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "rotation_id": "kms-rotation-1",
      "rotated_count": 1
    },
    "rotated_count": 1,
    "catalog_updated_count": 0,
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-catalog-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-catalog-negative.out 2>/tmp/mandoforge-stage2-archive-vault-catalog-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero KMS catalog update evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/vault-kms-rotation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "external_execution": {
      "status": "validated",
      "production_backend": true,
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "rotation_id": "kms-rotation-1",
      "rotated_count": 0
    },
    "rotated_count": 0,
    "catalog_updated_count": 1,
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-rotation-count-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-rotation-count-negative.out 2>/tmp/mandoforge-stage2-archive-vault-rotation-count-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero KMS rotated count to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/vault-kms-rotation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "external_execution": {
      "status": "validated",
      "production_backend": true,
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "rotation_id": "kms-rotation-1",
      "rotated_count": 1
    },
    "rotated_count": 1,
    "catalog_updated_count": 1,
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  cat >"$tmpdir/evidence/vault-kms-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "recovery_id": "kms-recovery-1",
      "recovery_target_kind": "production_kms_backend",
      "steps": []
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-recovery-steps-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-recovery-steps-negative.out 2>/tmp/mandoforge-stage2-archive-vault-recovery-steps-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing KMS recovery steps to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/vault-kms-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "recovery_id": "kms-recovery-1",
      "recovery_target_kind": "production_kms_backend",
      "steps": [
        {"name": "restore-key-material", "status": "validated"},
        {"name": "verify-secret-consumers", "status": "passed"}
      ]
    }
  }
}
JSON
  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": ["tenant-a", "tenant-b"],
      "rls_enforced": true,
      "rls_table_count": 12,
      "rls_forced_table_count": 8,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 3
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-rls-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-rls-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-rls-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected incomplete forced-RLS evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": ["tenant-a", "tenant-b"],
      "rls_enforced": true,
      "rls_table_count": 12,
      "rls_forced_table_count": 12,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": false,
      "cross_tenant_negative_test_count": 0
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-negative-tests-missing.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-negative-tests-missing.out 2>/tmp/mandoforge-stage2-archive-tenant-negative-tests-missing.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing cross-tenant negative tests to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": ["tenant-a"],
      "rls_enforced": true,
      "rls_table_count": 12,
      "rls_forced_table_count": 12,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 3
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-sample-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-sample-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-sample-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected single tenant sample evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": ["tenant-a", "tenant-b"],
      "rls_enforced": true,
      "rls_table_count": 12,
      "rls_forced_table_count": 12,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 3
    }
  }
}
JSON
  cat >"$tmpdir/evidence/policy-rollout-orchestration-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "target_kind": "production_policy_controller",
      "environment": "production",
      "controller_id": "policy-controller-prod-1",
      "rollout_scope": "global",
      "production_policy_store": true,
      "rollback_supported": false,
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "status": "passed"},
        {"name": "staged-runtime-clear", "status": "passed"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-rollback-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-rollback-negative.out 2>/tmp/mandoforge-stage2-archive-policy-rollback-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected policy rollout without rollback support to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-orchestration-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "target_kind": "production_policy_controller",
      "environment": "production",
      "controller_id": "policy-controller-prod-1",
      "rollout_scope": "global",
      "production_policy_store": true,
      "rollback_supported": true,
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": []
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-steps-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-steps-negative.out 2>/tmp/mandoforge-stage2-archive-policy-steps-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy rollout steps to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-orchestration-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "target_kind": "production_policy_controller",
      "environment": "production",
      "controller_id": "policy-controller-prod-1",
      "rollout_scope": "global",
      "production_policy_store": true,
      "rollback_supported": true,
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "status": "passed"},
        {"name": "staged-runtime-clear", "status": "passed"}
      ]
    }
  }
}
JSON
  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "scanned_count": 0,
    "skipped_count": 0,
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-due-run-scan-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-due-run-scan-negative.out 2>/tmp/mandoforge-stage2-archive-policy-due-run-scan-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero policy due-run scan count to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "scanned_count": 1,
    "skipped_count": 1,
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "different-prod-cluster",
      "replacement_scope": "cluster"
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-cluster-mismatch.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-cluster-negative.out 2>/tmp/mandoforge-stage2-archive-cluster-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected cluster-mismatched evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "cluster",
      "replacement_pods_healthy": true,
      "checked_pod_count": 1
    }
  }
}
JSON
  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "different-prod-cluster"
  },
  "remote_computer": {
    "state_sync_cluster_id": "different-prod-cluster",
    "sidecar_cluster_id": "different-prod-cluster",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "replacement_pods_healthy": true,
    "checked_pod_count": 1
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-mismatch.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-negative.out 2>/tmp/mandoforge-stage2-archive-summary-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected combined summary cluster mismatch to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1"
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "replacement_pods_healthy": true,
    "checked_pod_count": 1
  }
}
JSON

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "whiskey-pilot-cluster"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1"
    },
    "vault_kms": {
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1"
    },
    "finance": {
      "system_id": "netsuite-prod-1"
    },
    "managed_session_runtime": {
      "target_id": "managed-session-runtime-prod-1"
    }
  }
}
JSON
  cat >"$tmpdir/evidence/worker-load-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "whiskey-pilot-cluster",
      "load_validated": true,
      "isolated_worker_pool_configured": true
    }
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "controller_execution": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "whiskey-pilot-cluster",
      "distributed_state_backend": "juicefs",
      "state_claim": "mandoforge-remote-computer-state",
      "checked_path_count": 6
    }
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-sidecar-recovery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "validation_result": {
      "status": "validated",
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "whiskey-pilot-cluster",
      "replacement_scope": "cluster",
      "replacement_pods_healthy": true,
      "checked_pod_count": 1
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-pilot-identity.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-pilot-identity-negative.out 2>/tmp/mandoforge-stage2-archive-pilot-identity-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected pilot target identity evidence to fail" >&2
    exit 1
  fi
}

if [[ "$archive_path" == "--self-test" ]]; then
  self_test
  echo "stage2 evidence archive verifier self-test ok"
  exit 0
fi

if [[ -z "$archive_path" ]]; then
  echo "usage: scripts/verify-stage2-evidence-archive.sh <archive.tar.gz>" >&2
  echo "       scripts/verify-stage2-evidence-archive.sh --self-test" >&2
  exit 1
fi

verify_archive "$archive_path"
echo "stage2 evidence archive verified: $archive_path"
