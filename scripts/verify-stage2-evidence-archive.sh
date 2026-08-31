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

worker_load_check_detail_count() {
  jq -r '
    (.response.controller_execution.cluster_id // "") as $cluster_id
    | (.response.controller_execution.worker_pool // .response.controller_execution.pool_id // .response.controller_execution.queue // .response.controller_execution.queue_name // "") as $root_worker_pool
    | [
    (
      .response.controller_execution.load_checks[]?,
      .response.controller_execution.worker_pool_checks[]?,
      .response.controller_execution.validation_checks[]?,
      .response.controller_execution.load_validation_checks[]?,
      .response.controller_execution.checks[]?
    )
    | select(
        type == "object"
        and ((.name // .check // .kind // "") | length > 0)
        and (($cluster_id // "") | length > 0)
        and (($root_worker_pool // "") | length > 0)
        and ((.cluster_id // .worker_cluster_id // .target_cluster_id // "") == $cluster_id)
        and ((.worker_pool // .pool_id // .queue // .queue_name // "") == $root_worker_pool)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [$cluster_id, $root_worker_pool, (.name // .check // .kind // "")] | @tsv
  ] | unique | length' "$1"
}

summary_worker_load_check_detail_count() {
  jq -r '
    (.worker.cluster_id // "") as $cluster_id
    | (.worker.worker_pool // .worker.pool_id // .worker.queue // .worker.queue_name // "") as $root_worker_pool
    | [
    (
      .worker.load_checks[]?,
      .worker.worker_pool_checks[]?,
      .worker.validation_checks[]?,
      .worker.load_validation_checks[]?,
      .worker.checks[]?
    )
    | select(
        type == "object"
        and ((.name // .check // .kind // "") | length > 0)
        and (($cluster_id // "") | length > 0)
        and (($root_worker_pool // "") | length > 0)
        and ((.cluster_id // .worker_cluster_id // .target_cluster_id // "") == $cluster_id)
        and ((.worker_pool // .pool_id // .queue // .queue_name // "") == $root_worker_pool)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [$cluster_id, $root_worker_pool, (.name // .check // .kind // "")] | @tsv
  ] | unique | length' "$1"
}

remote_state_checked_path_detail_count() {
  jq -r '(.response.controller_execution.state_claim // "") as $state_claim | (.response.controller_execution.cluster_id // "") as $cluster_id | [
    (
      .response.controller_execution.checked_paths[]?,
      .response.controller_execution.checked_state_paths[]?,
      .response.controller_execution.path_checks[]?
    )
    | select(
        type == "object"
        and ($state_claim | length > 0)
        and (($cluster_id // "") | length > 0)
        and ((.path // .state_path // .name // "") | length > 0)
        and ((.state_claim // .claim // .pvc // .persistent_volume_claim // "") == $state_claim)
        and ((.cluster_id // .state_sync_cluster_id // .target_cluster_id // "") == $cluster_id)
        and ((.status // .result // .health // "") | ascii_downcase | IN("passed", "validated", "completed", "ready", "exists", "mounted", "available", "ok", "healthy", "accessible", "readable", "writable"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [$cluster_id, $state_claim, (.path // .state_path // .name // "")] | @tsv
  ] | unique | length' "$1"
}

summary_checked_path_detail_count() {
  jq -r '(.remote_computer.state_claim // "") as $state_claim | (.remote_computer.state_sync_cluster_id // "") as $cluster_id | [
    (
      .remote_computer.checked_paths[]?,
      .remote_computer.checked_state_paths[]?,
      .remote_computer.path_checks[]?
    )
    | select(
        type == "object"
        and ($state_claim | length > 0)
        and (($cluster_id // "") | length > 0)
        and ((.path // .state_path // .name // "") | length > 0)
        and ((.state_claim // .claim // .pvc // .persistent_volume_claim // "") == $state_claim)
        and ((.cluster_id // .state_sync_cluster_id // .target_cluster_id // "") == $cluster_id)
        and ((.status // .result // .health // "") | ascii_downcase | IN("passed", "validated", "completed", "ready", "exists", "mounted", "available", "ok", "healthy", "accessible", "readable", "writable"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [$cluster_id, $state_claim, (.path // .state_path // .name // "")] | @tsv
  ] | unique | length' "$1"
}

sidecar_checked_pod_detail_count() {
  jq -r '(.response.validation_result.cluster_id // "") as $cluster_id | [
    (
      .response.validation_result.checked_pods[]?,
      .response.validation_result.replacement_pods[]?,
      .response.validation_result.pod_checks[]?
    )
    | select(
        type == "object"
        and ((.pod // .pod_name // .name // "") | length > 0)
        and (($cluster_id // "") | length > 0)
        and ((.cluster_id // .sidecar_cluster_id // .target_cluster_id // "") == $cluster_id)
        and ((.status // .phase // .health // "") | ascii_downcase | IN("running", "ready", "healthy", "succeeded", "validated"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [$cluster_id, (.pod // .pod_name // .name // "")] | @tsv
  ] | unique | length' "$1"
}

summary_sidecar_checked_pod_detail_count() {
  jq -r '(.remote_computer.sidecar_cluster_id // "") as $cluster_id | [
    (
      .remote_computer.checked_pods[]?,
      .remote_computer.replacement_pods[]?,
      .remote_computer.pod_checks[]?
    )
    | select(
        type == "object"
        and ((.pod // .pod_name // .name // "") | length > 0)
        and (($cluster_id // "") | length > 0)
        and ((.cluster_id // .sidecar_cluster_id // .target_cluster_id // "") == $cluster_id)
        and ((.status // .phase // .health // "") | ascii_downcase | IN("running", "ready", "healthy", "succeeded", "validated"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [$cluster_id, (.pod // .pod_name // .name // "")] | @tsv
  ] | unique | length' "$1"
}

is_nonnegative_integer() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

is_positive_integer() {
  [[ "$1" =~ ^[0-9]+$ && "$1" -gt 0 ]]
}

managed_session_detail_issue() {
  local artifact="$1"
  local pending_start
  local pending_end
  local processed_before
  local processed_after
  local original_thread_id
  local resumed_thread_id
  local active_worker_lease_id
  local stale_worker_lease_id
  local stale_rejection_reason
  local runtime_turn_id
  local final_message_evidence

  pending_start="$(jq -r '.session_loop.pending_event_seq_start // .session_loop.event_window.start // .session_loop.sequence_range.start // ""' "$artifact")"
  pending_end="$(jq -r '.session_loop.pending_event_seq_end // .session_loop.event_window.end // .session_loop.sequence_range.end // ""' "$artifact")"
  processed_before="$(jq -r '.resume.processed_event_seq_before_restart // .resume.processed_event_seq_before // ""' "$artifact")"
  processed_after="$(jq -r '.resume.processed_event_seq_after_resume // .resume.processed_event_seq_after // .resume.processed_event_seq // ""' "$artifact")"
  original_thread_id="$(jq -r '.thread_lineage.original_thread_id // .thread_lineage.before_restart_thread_id // ""' "$artifact")"
  resumed_thread_id="$(jq -r '.thread_lineage.resumed_thread_id // .thread_lineage.after_restart_thread_id // ""' "$artifact")"
  active_worker_lease_id="$(jq -r '.lease_fencing.active_worker_lease_id // .lease_fencing.valid_worker_lease_id // ""' "$artifact")"
  stale_worker_lease_id="$(jq -r '.lease_fencing.stale_worker_lease_id // .lease_fencing.rejected_worker_lease_id // ""' "$artifact")"
  stale_rejection_reason="$(jq -r '.lease_fencing.stale_rejection_reason // .lease_fencing.rejection_reason // ""' "$artifact")"
  runtime_turn_id="$(jq -r '.runtime_turn.turn_id // .runtime_turn.id // ""' "$artifact")"
  final_message_evidence="$(jq -r '.runtime_turn.final_message // .runtime_turn.final_message_text // .runtime_turn.final_message_artifact_id // .runtime_turn.final_artifact_id // ""' "$artifact")"

  if ! is_positive_integer "$pending_start" || ! is_positive_integer "$pending_end" || [[ "$pending_start" -gt "$pending_end" ]]; then
    printf 'session-loop event cursor window evidence incomplete'
    return 0
  fi
  if ! is_nonnegative_integer "$processed_before" || ! is_nonnegative_integer "$processed_after" || [[ "$processed_after" != "$processed_before" || "$processed_after" -lt "$pending_end" ]]; then
    printf 'processed event cursor sequence evidence incomplete'
    return 0
  fi
  if [[ -z "$original_thread_id" || -z "$resumed_thread_id" || "$original_thread_id" != "$resumed_thread_id" ]]; then
    printf 'thread lineage id evidence incomplete'
    return 0
  fi
  if [[ -z "$active_worker_lease_id" || -z "$stale_worker_lease_id" || "$active_worker_lease_id" == "$stale_worker_lease_id" || -z "$stale_rejection_reason" ]]; then
    printf 'lease fencing id evidence incomplete'
    return 0
  fi
  if [[ -z "$runtime_turn_id" || -z "$final_message_evidence" ]]; then
    printf 'runtime turn final message detail evidence incomplete'
    return 0
  fi

  return 1
}

tenant_negative_test_detail_count() {
  jq -r '. as $root | ($root.response.controller_execution.deployment_id // "") as $deployment_id | [
    (
      $root.response.controller_execution.tenant_samples[]?,
      $root.response.controller_execution.tenant_ids_sample[]?
    )
    | select(
        type == "object"
        and (($deployment_id // "") | length > 0)
        and ((.deployment_id // .tenant_deployment_id // .routing_deployment_id // "") == $deployment_id)
      )
    | (.tenant_id // .tenant // .id // .name // "")
    | select(length > 0)
  ] | unique as $sampled_tenants | [
    (
      $root.response.controller_execution.cross_tenant_negative_test_results[]?,
      $root.response.controller_execution.cross_tenant_negative_tests_detail[]?,
      $root.response.controller_execution.negative_tests[]?
    )
    | (.source_tenant // .from_tenant // .tenant_id // "") as $source_tenant
    | (.target_tenant // .to_tenant // .blocked_tenant_id // "") as $target_tenant
    | select(
        type == "object"
        and ($source_tenant | length > 0)
        and ($target_tenant | length > 0)
        and ($source_tenant != $target_tenant)
        and (($sampled_tenants | index($source_tenant)) != null)
        and (($sampled_tenants | index($target_tenant)) != null)
        and (($deployment_id // "") | length > 0)
        and ((.deployment_id // .tenant_deployment_id // .routing_deployment_id // "") == $deployment_id)
        and (
          ((.status // .result // .outcome // "") | ascii_downcase | IN("passed", "blocked", "denied", "rejected", "prevented", "forbidden"))
          or (.access_granted == false)
        )
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .tested_at // .timestamp // "") | length > 0)
      )
    | [$deployment_id, $source_tenant, $target_tenant] | @tsv
  ] | unique | length' "$1"
}

forced_rls_table_detail_count() {
  jq -r '.response.controller_execution.deployment_id as $deployment_id | [
    (
      .response.controller_execution.rls_tables[]?,
      .response.controller_execution.rls_table_details[]?,
      .response.controller_execution.rls_table_checks[]?,
      .response.controller_execution.forced_rls_tables[]?
    )
    | select(
        type == "object"
        and ((.table // .table_name // .relation // .name // "") | length > 0)
        and ((.schema // .namespace // "public") | length > 0)
        and ((.rls_enabled // .enabled // false) == true)
        and ((.rls_forced // .forced // .force_rls // false) == true)
        and (($deployment_id // "") | length > 0)
        and ((.deployment_id // .tenant_deployment_id // .routing_deployment_id // "") == $deployment_id)
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
      )
    | [(.schema // .namespace // "public"), (.table // .table_name // .relation // .name)] | @tsv
  ] | unique | length' "$1"
}

expected_tenant_rls_table_count() {
  jq -r '[.expected_targets.tenant_routing.rls_tables[]? | select(type == "string" and length > 0) | ascii_downcase] | unique | length' "$1" 2>/dev/null || echo "0"
}

expected_tenant_rls_table_coverage_count() {
  jq -r --slurpfile manifest "$2" '
    ($manifest[0].expected_targets.tenant_routing.rls_tables
      // []
      | map(select(type == "string" and length > 0) | ascii_downcase)
      | unique) as $expected
    | .response.controller_execution.deployment_id as $deployment_id
    | [
      (
        .response.controller_execution.rls_tables[]?,
        .response.controller_execution.rls_table_details[]?,
        .response.controller_execution.rls_table_checks[]?,
        .response.controller_execution.forced_rls_tables[]?
      )
      | select(
          type == "object"
          and ((.table // .table_name // .relation // .name // "") | length > 0)
          and ((.schema // .namespace // "public") | length > 0)
          and ((.rls_enabled // .enabled // false) == true)
          and ((.rls_forced // .forced // .force_rls // false) == true)
          and (($deployment_id // "") | length > 0)
          and ((.deployment_id // .tenant_deployment_id // .routing_deployment_id // "") == $deployment_id)
          and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
        )
      | (((.schema // .namespace // "public") + "." + (.table // .table_name // .relation // .name)) | ascii_downcase)
    ] | unique as $actual
    | [$expected[] | select(. as $table | $actual | index($table))]
    | unique
    | length' "$1" 2>/dev/null || echo "0"
}

tenant_sample_count() {
  jq -r 'if ((.response.controller_execution.tenant_samples // null) | type) == "array" then (.response.controller_execution.tenant_samples | length) elif ((.response.controller_execution.tenant_ids_sample // null) | type) == "array" then (.response.controller_execution.tenant_ids_sample | length) else 0 end' "$1"
}

unique_tenant_sample_count() {
  jq -r '[
    (
      .response.controller_execution.tenant_samples[]?,
      .response.controller_execution.tenant_ids_sample[]?
    )
    | if type == "object" then (.tenant_id // .tenant // .id // .name // "") elif type == "string" then . else "" end
    | select(length > 0)
  ] | unique | length' "$1"
}

tenant_sample_detail_count() {
  jq -r '.response.controller_execution.deployment_id as $deployment_id | [
    (
      .response.controller_execution.tenant_samples[]?,
      .response.controller_execution.tenant_ids_sample[]?
    )
    | select(
        type == "object"
        and ((.tenant_id // .tenant // .id // .name // "") | length > 0)
        and (($deployment_id // "") | length > 0)
        and ((.deployment_id // .tenant_deployment_id // .routing_deployment_id // "") == $deployment_id)
        and (
          ((.status // .result // .outcome // "") | ascii_downcase | IN("sampled", "validated", "passed", "observed", "checked"))
          or (.validated == true)
        )
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .sampled_at // .timestamp // "") | length > 0)
      )
  ] | length' "$1"
}

kms_rotation_detail_count() {
  jq -r '
    (.response.external_execution.backend_id // "") as $root_backend_id
    | (.response.external_execution.key_id // "") as $root_key_id
    | (.response.external_execution.rotation_id // "") as $root_rotation_id
    | [
    (
      .response.rotated_keys[]?,
      .response.rotation_details[]?,
      .response.key_rotations[]?,
      .response.external_execution.rotated_keys[]?,
      .response.external_execution.rotation_details[]?,
      .response.external_execution.key_rotations[]?
    )
    | select(
        type == "object"
        and ($root_backend_id | length > 0)
        and ($root_key_id | length > 0)
        and ($root_rotation_id | length > 0)
        and ((.backend_id // .kms_backend_id // "") == $root_backend_id)
        and ((.key_id // .key // .kms_key_id // "") == $root_key_id)
        and ((.rotation_id // .rotation // .operation_id // "") == $root_rotation_id)
        and ((.catalog_updated // .catalog_update_confirmed // false) == true)
        and ((.status // .result // "") | ascii_downcase | IN("rotated", "validated", "completed", "passed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .rotated_at // .timestamp // "") | length > 0)
      )
    | [$root_backend_id, $root_key_id, $root_rotation_id, (.secret_id // .secret_ref // .record_id // .catalog_entry_id // .entry_id // .name // .path // .key_id // .key // .kms_key_id // "")] | @tsv
  ] | unique | length' "$1"
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

policy_due_run_scan_detail_count() {
  jq -r '
    (.response.controller_id // "") as $root_controller_id
    | (.response.policy_store_id // "") as $root_policy_store_id
    | (.response.deployment_id // "") as $root_deployment_id
    | [
    (
      .response.scanned_revisions[]?,
      .response.scanned_policies[]?,
      .response.policy_revisions[]?,
      .response.scanned_items[]?,
      .response.checked_revisions[]?
    )
    | select(
        type == "object"
        and ($root_controller_id | length > 0)
        and ($root_policy_store_id | length > 0)
        and ($root_deployment_id | length > 0)
        and ((.controller_id // .policy_controller_id // "") == $root_controller_id)
        and ((.policy_store_id // .store_id // "") == $root_policy_store_id)
        and ((.deployment_id // .policy_deployment_id // "") == $root_deployment_id)
        and ((.policy_id // .policy // .policy_key // .policy_name // "") | length > 0)
        and ((.revision_id // .revision // .policy_revision_id // .version // "") | length > 0)
        and ((.status // .result // .action // "") | ascii_downcase | IN("scanned", "checked", "skipped", "noop", "activated", "validated", "passed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .scanned_at // .timestamp // "") | length > 0)
      )
    | [$root_controller_id, $root_policy_store_id, $root_deployment_id, (.policy_id // .policy // .policy_key // .policy_name // ""), (.revision_id // .revision // .policy_revision_id // .version // "")] | @tsv
  ] | unique | length' "$1"
}

policy_rollout_step_detail_count() {
  jq -r '
    (.response.controller_execution.controller_id // "") as $root_controller_id
    | (.response.controller_execution.policy_store_id // "") as $root_policy_store_id
    | (.response.controller_execution.deployment_id // "") as $root_deployment_id
    | [
    .response.controller_execution.steps[]?
    | select(
        type == "object"
        and ($root_controller_id | length > 0)
        and ($root_policy_store_id | length > 0)
        and ($root_deployment_id | length > 0)
        and ((.controller_id // .policy_controller_id // "") == $root_controller_id)
        and ((.policy_store_id // .store_id // "") == $root_policy_store_id)
        and ((.deployment_id // .policy_deployment_id // "") == $root_deployment_id)
        and ((.name // .step // .kind // .action // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .timestamp // "") | length > 0)
      )
    | [$root_controller_id, $root_policy_store_id, $root_deployment_id, (.name // .step // .kind // .action // "")] | @tsv
  ] | unique | length' "$1"
}

kms_recovery_step_detail_count() {
  jq -r '
    (.response.controller_execution.backend_id // "") as $root_backend_id
    | (.response.controller_execution.key_id // "") as $root_key_id
    | (.response.controller_execution.recovery_id // "") as $root_recovery_id
    | [
    .response.controller_execution.steps[]?
    | select(
        type == "object"
        and ($root_backend_id | length > 0)
        and ($root_key_id | length > 0)
        and ($root_recovery_id | length > 0)
        and ((.backend_id // .kms_backend_id // "") == $root_backend_id)
        and ((.key_id // .kms_key_id // "") == $root_key_id)
        and ((.recovery_id // .recovery_run_id // "") == $root_recovery_id)
        and ((.name // .step // .kind // .action // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .timestamp // "") | length > 0)
      )
    | [$root_backend_id, $root_key_id, $root_recovery_id, (.name // .step // .kind // .action // "")] | @tsv
  ] | unique | length' "$1"
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

finance_delivery_receipt_count() {
  jq -r '
    (.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // "") as $root_system_id
    | (.export_state.latest_file_name // .export_state.file_name // .export_state.filename // "") as $root_file_name
    | ((.export_state.latest_bytes // .export_state.bytes // .export_state.export_bytes // 0) | tonumber? // 0) as $root_byte_count
    | [
      (
        .export_state.delivery_receipts[]?,
        .export_state.erp_delivery_receipts[]?,
        .export_state.accounting_receipts[]?,
        .export_state.deliveries[]?,
        .export_state.erp_batches[]?,
        if (((.export_state.latest_delivery_receipt_id // .export_state.latest_receipt_id // .export_state.latest_erp_batch_id // .export_state.latest_batch_id // "") | length) > 0) then
          {
            receipt_id: (.export_state.latest_delivery_receipt_id // .export_state.latest_receipt_id // .export_state.latest_erp_batch_id // .export_state.latest_batch_id),
            system_id: $root_system_id,
            file_name: $root_file_name,
            byte_count: $root_byte_count,
            status: (.export_state.latest_delivery_status // .export_state.latest_receipt_status // .export_state.latest_status // "unknown"),
            record_count: (.export_state.latest_record_count // .export_state.latest_posted_record_count // .export_state.posted_record_count // .export_state.latest_row_count // 0),
            audit_id: (.export_state.latest_delivery_audit_id // .export_state.latest_audit_id // .export_state.audit_id),
            run_id: (.export_state.latest_delivery_run_id // .export_state.latest_run_id // .export_state.run_id),
            posted_at: (.export_state.latest_posted_at // .export_state.latest_delivered_at // .export_state.latest_timestamp // .export_state.posted_at // .export_state.delivered_at // .export_state.timestamp)
          }
        else empty end
      )
      | select(
          type == "object"
          and ((.receipt_id // .receipt // .batch_id // .erp_batch_id // .delivery_id // .posting_id // "") | length > 0)
          and (((.system_id // .erp_system_id // .accounting_system_id // .target_id // "") as $receipt_system_id | ($receipt_system_id | length > 0) and $receipt_system_id == $root_system_id))
          and ($root_file_name | length > 0)
          and (((.file_name // .filename // .export_file_name // .csv_file_name // "") as $receipt_file_name | ($receipt_file_name | length > 0) and $receipt_file_name == $root_file_name))
          and ($root_byte_count > 0)
          and ((((.byte_count // .bytes // .export_bytes // .csv_bytes // 0) | tonumber? // 0) as $receipt_byte_count | $receipt_byte_count == $root_byte_count))
          and ((.status // .result // "") | ascii_downcase | IN("delivered", "posted", "accepted", "completed", "reconciled", "validated"))
          and (((.record_count // .posted_record_count // .line_count // .row_count // .entry_count // 0) | tonumber? // 0) > 0)
          and ((.audit_id // .audit_log_id // .trace_id // .run_id // .posted_at // .delivered_at // .received_at // .accepted_at // .timestamp // "") | length > 0)
        )
      | [$root_system_id, $root_file_name, ($root_byte_count | tostring), (.receipt_id // .receipt // .batch_id // .erp_batch_id // .delivery_id // .posting_id // "")] | @tsv
    ] | unique | length' "$1"
}

finance_close_step_detail_count() {
  jq -r '
    (.response.close_controller_execution.close_id // "") as $root_close_id
    | [
    .response.close_controller_execution.steps[]?
    | select(
        type == "object"
        and ($root_close_id | length > 0)
        and ((.close_id // .finance_close_id // "") == $root_close_id)
        and ((.name // .step // .kind // .action // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .timestamp // "") | length > 0)
      )
    | [$root_close_id, (.name // .step // .kind // .action // "")] | @tsv
  ] | unique | length' "$1"
}

finance_reconciliation_check_detail_count() {
  jq -r '
    (.response.reconciliation_id // "") as $root_reconciliation_id
    | [
    .response.checks[]?
    | select(
        type == "object"
        and ($root_reconciliation_id | length > 0)
        and ((.reconciliation_id // .finance_reconciliation_id // "") == $root_reconciliation_id)
        and ((.name // .check // .kind // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .timestamp // "") | length > 0)
      )
    | [$root_reconciliation_id, (.name // .check // .kind // "")] | @tsv
  ] | unique | length' "$1"
}

relative_artifact_path_safe() {
  local summary_path="$1"
  local segment
  local -a segments
  [[ -n "$summary_path" ]] || return 1
  [[ "$summary_path" != /* ]] || return 1
  IFS='/' read -r -a segments <<<"$summary_path"
  for segment in "${segments[@]}"; do
    [[ -n "$segment" && "$segment" != "." && "$segment" != ".." ]] || return 1
  done
}

enterprise_checklist_summary_paths_exist() {
  local root="$1"
  local checklist="$2"
  local summary_path
  local expected_source
  while IFS=$'\t' read -r summary_path expected_source; do
    if ! relative_artifact_path_safe "$summary_path"; then
      enterprise_checklist_summary_path_issue="unsafe summary_path=$summary_path"
      return 1
    fi
    if [[ ! -s "$root/$summary_path" ]]; then
      enterprise_checklist_summary_path_issue="missing summary_path=$summary_path"
      return 1
    fi
    jq -e --arg expected_source "$expected_source" '
      (.source // "") == $expected_source
      and (.required_evidence_class // "") == "customer_grade"
      and ((.status // "") | IN("ready", "validated", "completed", "passed"))
      and ((.blocked_count // 0) == 0)
    ' "$root/$summary_path" >/dev/null || {
      enterprise_checklist_summary_path_issue="summary_path=$summary_path does not match expected_source=$expected_source, customer_grade, ready status, or blocked_count=0"
      return 1
    }
  done < <(jq -r '.lane_results[]? | select((.status // "") == "ready") | [.summary_path // "", .expected_source // ""] | @tsv' "$checklist")
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
    stage2-production-evidence-preflight.json)
      jq -e '
        . as $root
        | ($root.source // "") == "stage2-production-evidence-preflight"
        and ($root.status // "") == "passed"
        and (($root.fail_count // 1) == 0)
        and (($root.pass_count // 0) > 0)
        and (($root.checks // []) | length == ($root.pass_count // -1))
        and all($root.checks[]?; (.status // "") == "passed" and ((.scope // "") | length > 0) and ((.detail // "") | length > 0))
      ' "$path" >/dev/null || {
        printf '%s does not prove strict Stage 2 production evidence preflight success' "$relative_path"
        return 0
      }
      ;;
    worker-load-validation-evidence.json)
      local evidence_status
      local controller_status
      local target_kind
      local node_count
      local cluster_id
      local load_validated
      local isolated_worker_pool_configured
      local worker_pool
      local load_check_count
      local load_check_detail_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.controller_execution.node_count // 0' "$path")"
      cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$path")"
      load_validated="$(jq -r '.response.controller_execution.load_validated // false' "$path")"
      isolated_worker_pool_configured="$(jq -r '.response.controller_execution.isolated_worker_pool_configured // false' "$path")"
      worker_pool="$(jq -r '.response.controller_execution.worker_pool // .response.controller_execution.pool_id // .response.controller_execution.queue // .response.controller_execution.queue_name // ""' "$path")"
      load_check_count="$(jq -r '.response.controller_execution.load_check_count // .response.controller_execution.worker_pool_check_count // .response.controller_execution.validation_check_count // .response.controller_execution.check_count // 0' "$path")"
      load_check_detail_count="$(worker_load_check_detail_count "$path")"
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
      if [[ ! "$load_check_detail_count" =~ ^[0-9]+$ || "$load_check_detail_count" == "0" ]]; then
        printf '%s cluster_bound_load_check_detail_count=%s' "$relative_path" "$load_check_detail_count"
        return 0
      fi
      if [[ "$load_check_count" =~ ^[0-9]+$ && "$load_check_count" -gt 0 ]]; then
        if [[ ! "$load_check_detail_count" =~ ^[0-9]+$ || "$load_check_detail_count" -lt "$load_check_count" ]]; then
          printf '%s cluster_bound_unique_load_check_detail_count=%s load_check_count=%s' "$relative_path" "$load_check_detail_count" "$load_check_count"
          return 0
        fi
      fi
      if [[ "$isolated_worker_pool_configured" != "true" ]]; then
        printf '%s isolated_worker_pool_configured=%s' "$relative_path" "$isolated_worker_pool_configured"
        return 0
      fi
      if [[ -z "$worker_pool" ]]; then
        printf '%s worker_pool identity missing' "$relative_path"
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
      local checked_path_detail_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.controller_execution.node_count // 0' "$path")"
      state_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // "unknown"' "$path")"
      cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$path")"
      state_claim="$(jq -r '.response.controller_execution.state_claim // ""' "$path")"
      checked_path_count="$(jq -r '.response.controller_execution.checked_path_count // 0' "$path")"
      checked_path_detail_count="$(remote_state_checked_path_detail_count "$path")"
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
      if [[ ! "$checked_path_detail_count" =~ ^[0-9]+$ || "$checked_path_detail_count" -lt "$checked_path_count" ]]; then
        printf '%s cluster_bound_checked_path_detail_count=%s checked_path_count=%s' "$relative_path" "$checked_path_detail_count" "$checked_path_count"
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
      local checked_pod_detail_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      status="$(jq -r '.response.validation_result.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.validation_result.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.validation_result.node_count // 0' "$path")"
      replacement_scope="$(jq -r '.response.validation_result.replacement_scope // "unknown"' "$path")"
      cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$path")"
      replacement_pods_healthy="$(jq -r '.response.validation_result.replacement_pods_healthy // false' "$path")"
      checked_pod_count="$(jq -r '.response.validation_result.checked_pod_count // 0' "$path")"
      checked_pod_detail_count="$(sidecar_checked_pod_detail_count "$path")"
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
      if [[ ! "$checked_pod_detail_count" =~ ^[0-9]+$ || "$checked_pod_detail_count" -lt "$checked_pod_count" ]]; then
        printf '%s cluster_bound_checked_pod_detail_count=%s checked_pod_count=%s' "$relative_path" "$checked_pod_detail_count" "$checked_pod_count"
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
      local state_checked_path_detail_count
      local worker_load_check_detail_count
      local summary_worker_load_check_detail_count
      local sidecar_replacement_pods_healthy
      local sidecar_checked_pod_count
      local sidecar_checked_pod_detail_count
      local worker_pool
      summary_status="$(jq -r '.status // "unknown"' "$path")"
      production_blocked="$(jq -r 'if has("production_blocked") then .production_blocked else true end' "$path")"
      same_cluster_target="$(jq -r '.same_cluster_target // false' "$path")"
      worker_cluster_id="$(jq -r '.worker.cluster_id // ""' "$path")"
      worker_pool="$(jq -r '.worker.worker_pool // .worker.pool_id // .worker.queue // .worker.queue_name // ""' "$path")"
      state_cluster_id="$(jq -r '.remote_computer.state_sync_cluster_id // ""' "$path")"
      sidecar_cluster_id="$(jq -r '.remote_computer.sidecar_cluster_id // ""' "$path")"
      state_backend="$(jq -r '.remote_computer.distributed_state_backend // "unknown"' "$path")"
      state_claim="$(jq -r '.remote_computer.state_claim // ""' "$path")"
      state_checked_path_count="$(jq -r '.remote_computer.checked_path_count // 0' "$path")"
      state_checked_path_detail_count="$(summary_checked_path_detail_count "$path")"
      worker_load_check_detail_count="$(jq -r '.worker.load_check_detail_count // 0' "$path")"
      summary_worker_load_check_detail_count="$(summary_worker_load_check_detail_count "$path")"
      sidecar_replacement_pods_healthy="$(jq -r '.remote_computer.replacement_pods_healthy // false' "$path")"
      sidecar_checked_pod_count="$(jq -r '.remote_computer.checked_pod_count // 0' "$path")"
      sidecar_checked_pod_detail_count="$(summary_sidecar_checked_pod_detail_count "$path")"
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
      if [[ -z "$worker_pool" ]]; then
        printf '%s worker_pool identity missing' "$relative_path"
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
      if [[ ! "$state_checked_path_detail_count" =~ ^[0-9]+$ || "$state_checked_path_detail_count" -lt "$state_checked_path_count" ]]; then
        printf '%s cluster_bound_checked_path_detail_count=%s checked_path_count=%s' "$relative_path" "$state_checked_path_detail_count" "$state_checked_path_count"
        return 0
      fi
      if [[ ! "$worker_load_check_detail_count" =~ ^[0-9]+$ || "$worker_load_check_detail_count" == "0" ]]; then
        printf '%s worker_load_check_detail_count=%s' "$relative_path" "$worker_load_check_detail_count"
        return 0
      fi
      if [[ ! "$summary_worker_load_check_detail_count" =~ ^[0-9]+$ || "$summary_worker_load_check_detail_count" -lt "$worker_load_check_detail_count" ]]; then
        printf '%s cluster_bound_summary_worker_load_check_detail_count=%s worker_load_check_detail_count=%s' "$relative_path" "$summary_worker_load_check_detail_count" "$worker_load_check_detail_count"
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
      if [[ ! "$sidecar_checked_pod_detail_count" =~ ^[0-9]+$ || "$sidecar_checked_pod_detail_count" -lt "$sidecar_checked_pod_count" ]]; then
        printf '%s cluster_bound_sidecar_checked_pod_detail_count=%s checked_pod_count=%s' "$relative_path" "$sidecar_checked_pod_detail_count" "$sidecar_checked_pod_count"
        return 0
      fi
      ;;
    remote-computer-session-pod-lifecycle-evidence.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.remote-computer-production-state-gate" SOURCE_EVIDENCE_DIR="$root" REMOTE_COMPUTER_EVIDENCE_DIR="$root/remote-computer" WORKER_REMOTE_COMPUTER_EVIDENCE_DIR="$root/worker-remote-computer" REMOTE_COMPUTER_SESSION_POD_LIFECYCLE_EVIDENCE_FILE="$path" scripts/remote-computer-production-state-gate.sh 2>&1)"; then
        printf '%s rejected by Remote Computer production state gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    tenant-routing-validation-evidence.json)
      local evidence_status
      local validation_status
      local target_kind
      local tenant_count
      local tenant_sample_count
      local unique_tenant_sample_count
      local tenant_sample_detail_count
      local rls_enforced
      local rls_table_count
      local rls_forced_table_count
      local rls_table_detail_count
      local expected_rls_table_count
      local expected_rls_table_coverage_count
      local tenant_context_validated
      local cross_tenant_negative_tests
      local cross_tenant_negative_test_count
      local cross_tenant_negative_test_detail_count
      local deployment_id
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      validation_status="$(jq -r '.response.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      tenant_count="$(jq -r '.response.controller_execution.tenant_count // 0' "$path")"
      tenant_sample_count="$(tenant_sample_count "$path")"
      unique_tenant_sample_count="$(unique_tenant_sample_count "$path")"
      tenant_sample_detail_count="$(tenant_sample_detail_count "$path")"
      rls_enforced="$(jq -r '.response.controller_execution.rls_enforced // false' "$path")"
      rls_table_count="$(jq -r '.response.controller_execution.rls_table_count // .response.controller_execution.rls_enabled_table_count // 0' "$path")"
      rls_forced_table_count="$(jq -r '.response.controller_execution.rls_forced_table_count // .response.controller_execution.forced_rls_table_count // 0' "$path")"
      rls_table_detail_count="$(forced_rls_table_detail_count "$path")"
      expected_rls_table_count="$(expected_tenant_rls_table_count "$root/production-evidence-run.json")"
      expected_rls_table_coverage_count="$(expected_tenant_rls_table_coverage_count "$path" "$root/production-evidence-run.json")"
      tenant_context_validated="$(jq -r '.response.controller_execution.tenant_context_validated // false' "$path")"
      cross_tenant_negative_tests="$(jq -r '.response.controller_execution.cross_tenant_negative_tests // false' "$path")"
      cross_tenant_negative_test_count="$(jq -r '.response.controller_execution.cross_tenant_negative_test_count // .response.controller_execution.negative_test_count // 0' "$path")"
      cross_tenant_negative_test_detail_count="$(tenant_negative_test_detail_count "$path")"
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
      if [[ ! "$unique_tenant_sample_count" =~ ^[0-9]+$ || "$unique_tenant_sample_count" -lt 2 ]]; then
        printf '%s unique_tenant_sample_count=%s is not an audited multi-tenant sample' "$relative_path" "$unique_tenant_sample_count"
        return 0
      fi
      if [[ ! "$tenant_sample_detail_count" =~ ^[0-9]+$ || "$tenant_sample_detail_count" -lt "$tenant_sample_count" ]]; then
        printf '%s deployment_bound_tenant_sample_detail_count=%s tenant_sample_count=%s' "$relative_path" "$tenant_sample_detail_count" "$tenant_sample_count"
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
      if [[ ! "$rls_table_detail_count" =~ ^[0-9]+$ || "$rls_table_detail_count" -lt "$rls_table_count" ]]; then
        printf '%s deployment_bound_unique_forced_rls_table_detail_count=%s rls_table_count=%s' "$relative_path" "$rls_table_detail_count" "$rls_table_count"
        return 0
      fi
      if [[ "$expected_rls_table_count" =~ ^[0-9]+$ && "$expected_rls_table_count" -gt 0 ]]; then
        if [[ ! "$expected_rls_table_coverage_count" =~ ^[0-9]+$ || "$expected_rls_table_coverage_count" -lt "$expected_rls_table_count" ]]; then
          printf '%s expected_forced_rls_table_coverage_count=%s expected_rls_table_count=%s' "$relative_path" "$expected_rls_table_coverage_count" "$expected_rls_table_count"
          return 0
        fi
      fi
      if [[ ! "$cross_tenant_negative_test_count" =~ ^[0-9]+$ || "$cross_tenant_negative_test_count" == "0" ]]; then
        printf '%s cross_tenant_negative_test_count=%s' "$relative_path" "$cross_tenant_negative_test_count"
        return 0
      fi
      if [[ ! "$cross_tenant_negative_test_detail_count" =~ ^[0-9]+$ || "$cross_tenant_negative_test_detail_count" -lt "$cross_tenant_negative_test_count" ]]; then
        printf '%s deployment_bound_sampled_tenant_negative_test_detail_count=%s cross_tenant_negative_test_count=%s' "$relative_path" "$cross_tenant_negative_test_detail_count" "$cross_tenant_negative_test_count"
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
      local rollback_evidence_id
      local rollback_audit_evidence
      local policy_store_id
      local deployment_id
      local step_count
      local step_detail_count
      local invalid_step_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      status="$(jq -r '.response.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$path")"
      controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$path")"
      rollout_scope="$(jq -r '.response.controller_execution.rollout_scope // "unknown"' "$path")"
      production_policy_store="$(jq -r '.response.controller_execution.production_policy_store // false' "$path")"
      rollback_supported="$(jq -r '.response.controller_execution.rollback_supported // false' "$path")"
      rollback_evidence_id="$(jq -r '.response.controller_execution.rollback_plan_id // .response.controller_execution.rollback_procedure_id // .response.controller_execution.rollback_strategy_id // .response.controller_execution.rollback_revision_id // .response.controller_execution.rollback_run_id // ""' "$path")"
      rollback_audit_evidence="$(jq -r '.response.controller_execution.rollback_audit_id // .response.controller_execution.rollback_trace_id // .response.controller_execution.rollback_run_audit_id // .response.controller_execution.rollback_checked_at // .response.controller_execution.rollback_validated_at // ""' "$path")"
      policy_store_id="$(jq -r '.response.controller_execution.policy_store_id // ""' "$path")"
      deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$path")"
      step_count="$(jq -r 'if ((.response.controller_execution.steps // null) | type) == "array" then (.response.controller_execution.steps | length) else 0 end' "$path")"
      step_detail_count="$(policy_rollout_step_detail_count "$path")"
      invalid_step_count="$(jq -r '[.response.controller_execution.steps[]? | select((.status // "") as $status | ($status != "passed" and $status != "validated" and $status != "completed"))] | length' "$path")"
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
      if ! is_production_identity_value "$rollback_evidence_id"; then
        printf '%s rollback_evidence_id=%s is a pilot/mock/local identity' "$relative_path" "${rollback_evidence_id:-<empty>}"
        return 0
      fi
      if [[ -z "$rollback_audit_evidence" ]]; then
        printf '%s rollback audit or trace evidence is missing' "$relative_path"
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
      if [[ ! "$step_detail_count" =~ ^[0-9]+$ || "$step_detail_count" -lt "$step_count" ]]; then
        printf '%s step_detail_count=%s step_count=%s missing controller/policy store/deployment binding' "$relative_path" "$step_detail_count" "$step_count"
        return 0
      fi
      if [[ ! "$invalid_step_count" =~ ^[0-9]+$ || "$invalid_step_count" != "0" ]]; then
        printf '%s invalid policy rollout step status count=%s' "$relative_path" "$invalid_step_count"
        return 0
      fi
      ;;
    policy-rollout-due-run-evidence.json)
      local evidence_status
      local due_run_status
      local scanned_count
      local scan_detail_count
      local checked_at
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      due_run_status="$(jq -r '.response.status // "unknown"' "$path")"
      scanned_count="$(jq -r '.response.scanned_count // 0' "$path")"
      scan_detail_count="$(policy_due_run_scan_detail_count "$path")"
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
      if [[ ! "$scan_detail_count" =~ ^[0-9]+$ || "$scan_detail_count" -lt "$scanned_count" ]]; then
        printf '%s scan_detail_count=%s scanned_count=%s missing controller/policy store/deployment binding' "$relative_path" "$scan_detail_count" "$scanned_count"
        return 0
      fi
      if [[ -z "$checked_at" ]]; then
        printf '%s checked_at is missing' "$relative_path"
        return 0
      fi
      ;;
    provider-production-rollout-evidence.json)
      local evidence_status
      local rollout_status
      local environment
      local provider_count
      local provider_id_count
      local enforcement_blocked
      local controller_configured
      local controller_status
      local rollout_id
      local ran_at
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      rollout_status="$(jq -r '.response.status // "unknown"' "$path")"
      environment="$(jq -r '.response.environment // "unknown"' "$path")"
      provider_count="$(jq -r '.response.provider_count // 0' "$path")"
      provider_id_count="$(jq -r '[.response.provider_ids[]?] | length' "$path")"
      enforcement_blocked="$(jq -r '.response.enforcement.production_blocked // true' "$path")"
      controller_configured="$(jq -r '.response.controller_configured // false' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      rollout_id="$(jq -r '.response.id // ""' "$path")"
      ran_at="$(jq -r '.response.ran_at // ""' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$rollout_status" != "applied" || "$controller_status" != "applied" ]]; then
        printf '%s status=%s controller_status=%s' "$relative_path" "$rollout_status" "$controller_status"
        return 0
      fi
      if [[ "$environment" != "production" && "$environment" != "prod" ]]; then
        printf '%s environment=%s is not production' "$relative_path" "$environment"
        return 0
      fi
      if [[ "$controller_configured" != "true" || "$enforcement_blocked" != "false" ]]; then
        printf '%s controller or enforcement evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ ! "$provider_count" =~ ^[0-9]+$ || "$provider_count" == "0" || "$provider_id_count" != "$provider_count" ]]; then
        printf '%s provider_count=%s provider_id_count=%s' "$relative_path" "$provider_count" "$provider_id_count"
        return 0
      fi
      if [[ -z "$rollout_id" || -z "$ran_at" ]]; then
        printf '%s lacks audit id or timestamp' "$relative_path"
        return 0
      fi
      ;;
    provider-production-rollback-evidence.json)
      local evidence_status
      local rollback_status
      local environment
      local provider_count
      local provider_id_count
      local controller_configured
      local controller_status
      local rollback_id
      local source_rollout_id
      local ran_at
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      rollback_status="$(jq -r '.response.status // "unknown"' "$path")"
      environment="$(jq -r '.response.environment // "unknown"' "$path")"
      provider_count="$(jq -r '.response.provider_count // 0' "$path")"
      provider_id_count="$(jq -r '[.response.provider_ids[]?] | length' "$path")"
      controller_configured="$(jq -r '.response.controller_configured // false' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      rollback_id="$(jq -r '.response.id // ""' "$path")"
      source_rollout_id="$(jq -r '.response.source_rollout_id // ""' "$path")"
      ran_at="$(jq -r '.response.ran_at // ""' "$path")"
      if [[ "$evidence_status" != "captured" ]]; then
        printf '%s evidence_status=%s' "$relative_path" "$evidence_status"
        return 0
      fi
      if [[ "$rollback_status" != "rolled_back" || "$controller_status" != "rolled_back" ]]; then
        printf '%s status=%s controller_status=%s' "$relative_path" "$rollback_status" "$controller_status"
        return 0
      fi
      if [[ "$environment" != "production" && "$environment" != "prod" ]]; then
        printf '%s environment=%s is not production' "$relative_path" "$environment"
        return 0
      fi
      if [[ "$controller_configured" != "true" ]]; then
        printf '%s controller evidence incomplete' "$relative_path"
        return 0
      fi
      if [[ ! "$provider_count" =~ ^[0-9]+$ || "$provider_count" == "0" || "$provider_id_count" != "$provider_count" ]]; then
        printf '%s provider_count=%s provider_id_count=%s' "$relative_path" "$provider_count" "$provider_id_count"
        return 0
      fi
      if [[ -z "$rollback_id" || -z "$source_rollout_id" || -z "$ran_at" ]]; then
        printf '%s lacks audit id, source rollout id, or timestamp' "$relative_path"
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
      local rotation_detail_count
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
      rotation_detail_count="$(kms_rotation_detail_count "$path")"
      action_count="$(jq -r '[.response.actions[]? | select(. == "external_kms_rotation_confirmed")] | length' "$path")"
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
      if [[ ! "$rotation_detail_count" =~ ^[0-9]+$ || "$rotation_detail_count" -lt "$rotated_count" || "$rotation_detail_count" -lt "$catalog_updated_count" ]]; then
        printf '%s rotation_detail_count=%s rotated_count=%s catalog_updated_count=%s' "$relative_path" "$rotation_detail_count" "$rotated_count" "$catalog_updated_count"
        return 0
      fi
      if [[ ! "$action_count" =~ ^[0-9]+$ || "$action_count" == "0" ]]; then
        printf '%s external KMS rotation confirmation action missing' "$relative_path"
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
      local step_detail_count
      local invalid_step_count
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
      step_detail_count="$(kms_recovery_step_detail_count "$path")"
      invalid_step_count="$(jq -r '[.response.controller_execution.steps[]? | select((.status // "") as $status | ($status != "passed" and $status != "validated" and $status != "completed"))] | length' "$path")"
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
      if [[ ! "$step_detail_count" =~ ^[0-9]+$ || "$step_detail_count" -lt "$step_count" ]]; then
        printf '%s bound_recovery_step_detail_count=%s step_count=%s' "$relative_path" "$step_detail_count" "$step_count"
        return 0
      fi
      if [[ ! "$invalid_step_count" =~ ^[0-9]+$ || "$invalid_step_count" != "0" ]]; then
        printf '%s invalid recovery step status count=%s' "$relative_path" "$invalid_step_count"
        return 0
      fi
      ;;
    finance-export-delivery-observer.json)
      local status
      local delivery_mode
      local delivery_count
      local delivery_receipt_count
      local system_id
      status="$(jq -r '.status // "unknown"' "$path")"
      delivery_mode="$(jq -r '.export_state.delivery_mode // "unknown"' "$path")"
      delivery_count="$(jq -r '.export_state.delivery_count // 0' "$path")"
      delivery_receipt_count="$(finance_delivery_receipt_count "$path")"
      system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$path")"
      if [[ "$status" != "ok" ]]; then
        printf '%s observer_status=%s' "$relative_path" "$status"
        return 0
      fi
      if [[ ! "$delivery_count" =~ ^[0-9]+$ || "$delivery_count" == "0" ]]; then
        printf '%s delivery_count=%s' "$relative_path" "$delivery_count"
        return 0
      fi
      if [[ ! "$delivery_receipt_count" =~ ^[0-9]+$ || "$delivery_receipt_count" -lt "$delivery_count" ]]; then
        printf '%s delivery_receipt_count=%s delivery_count=%s missing current export file binding' "$relative_path" "$delivery_receipt_count" "$delivery_count"
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
      local step_detail_count
      local invalid_step_count
      local action_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      run_status="$(jq -r '.response.status // "unknown"' "$path")"
      close_configured="$(jq -r '.response.close_controller_configured // false' "$path")"
      close_status="$(jq -r '.response.close_controller_execution.status // "unknown"' "$path")"
      close_id="$(jq -r '.response.close_controller_execution.close_id // ""' "$path")"
      step_count="$(jq -r 'if ((.response.close_controller_execution.steps // null) | type) == "array" then (.response.close_controller_execution.steps | length) else 0 end' "$path")"
      step_detail_count="$(finance_close_step_detail_count "$path")"
      invalid_step_count="$(jq -r '[.response.close_controller_execution.steps[]? | select((.status // "") as $status | ($status != "passed" and $status != "validated" and $status != "completed"))] | length' "$path")"
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
      if [[ ! "$step_detail_count" =~ ^[0-9]+$ || "$step_detail_count" -lt "$step_count" ]]; then
        printf '%s step_detail_count=%s step_count=%s' "$relative_path" "$step_detail_count" "$step_count"
        return 0
      fi
      if [[ ! "$invalid_step_count" =~ ^[0-9]+$ || "$invalid_step_count" != "0" ]]; then
        printf '%s invalid finance close step status count=%s' "$relative_path" "$invalid_step_count"
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
      local check_detail_count
      local invalid_check_count
      evidence_status="$(jq -r '.status // "unknown"' "$path")"
      reconciliation_status="$(jq -r '.response.status // "unknown"' "$path")"
      reconciliation_id="$(jq -r '.response.reconciliation_id // ""' "$path")"
      check_count="$(jq -r 'if ((.response.checks // null) | type) == "array" then (.response.checks | length) else 0 end' "$path")"
      check_detail_count="$(finance_reconciliation_check_detail_count "$path")"
      invalid_check_count="$(jq -r '[.response.checks[]? | select((.status // "") as $status | ($status != "passed" and $status != "validated" and $status != "completed"))] | length' "$path")"
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
      if [[ ! "$check_detail_count" =~ ^[0-9]+$ || "$check_detail_count" -lt "$check_count" ]]; then
        printf '%s check_detail_count=%s check_count=%s' "$relative_path" "$check_detail_count" "$check_count"
        return 0
      fi
      if [[ ! "$invalid_check_count" =~ ^[0-9]+$ || "$invalid_check_count" != "0" ]]; then
        printf '%s invalid finance reconciliation check status count=%s' "$relative_path" "$invalid_check_count"
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
      local detail_issue
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
      detail_issue=""
      if detail_issue="$(managed_session_detail_issue "$path")"; then
        printf '%s %s' "$relative_path" "$detail_issue"
        return 0
      fi
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
    product-surfaces/summary.json)
      local summary_status
      local evidence_class
      local target_id
      local target_kind
      local target_environment
      local target_base_url
      local target_git_sha
      local target_image_tag
      local audit_id
      local checked_at
      local support_owner
      local archive_uri
      local immutable
      local archive_digest
      local retention_policy

      summary_status="$(jq -r '.status // "unknown"' "$path")"
      evidence_class="$(jq -r '.evidence_class // .required_evidence_class // ""' "$path")"
      target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$path")"
      target_kind="$(jq -r '.target.kind // "unknown"' "$path")"
      target_environment="$(jq -r '.target.environment // ""' "$path")"
      target_base_url="$(jq -r '.target.base_url // ""' "$path")"
      target_git_sha="$(jq -r '.target.git_sha // ""' "$path")"
      target_image_tag="$(jq -r '.target.image_tag // ""' "$path")"
      audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$path")"
      checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$path")"
      support_owner="$(jq -r '.support_owner // .product_owner // .oncall_owner // ""' "$path")"
      archive_uri="$(jq -r '.evidence_archive.uri // .archive.uri // ""' "$path")"
      immutable="$(jq -r '.evidence_archive.immutable // .archive.immutable // false' "$path")"
      archive_digest="$(jq -r '.evidence_archive.digest // .archive.digest // ""' "$path")"
      retention_policy="$(jq -r '.evidence_archive.retention_policy // .archive.retention_policy // ""' "$path")"

      case "$summary_status" in
        ready|validated|completed|passed) ;;
        *)
          printf '%s status=%s' "$relative_path" "$summary_status"
          return 0
          ;;
      esac
      if [[ "$evidence_class" != "customer_grade" ]]; then
        printf '%s evidence_class=%s is not customer_grade' "$relative_path" "${evidence_class:-<empty>}"
        return 0
      fi
      if ! is_production_identity_value "$target_id" || ! is_production_identity_value "$target_base_url"; then
        printf '%s target id or base_url is pilot/mock/local' "$relative_path"
        return 0
      fi
      if [[ "$target_environment" != "production" || -z "$target_git_sha" || -z "$target_image_tag" ]]; then
        printf '%s target lacks production environment, git SHA, or image tag' "$relative_path"
        return 0
      fi
      case "$target_kind" in
        production_product_surface|production_ui|customer_grade_deployment|kubernetes_cluster|managed_agent_cluster) ;;
        *)
          printf '%s target_kind=%s is not production-grade' "$relative_path" "$target_kind"
          return 0
          ;;
      esac
      if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
        printf '%s lacks audit id, timestamp, or support owner' "$relative_path"
        return 0
      fi
      if [[ "$immutable" != "true" || -z "$archive_uri" || -z "$archive_digest" || -z "$retention_policy" ]]; then
        printf '%s lacks immutable archive metadata' "$relative_path"
        return 0
      fi
      jq -e '
        . as $root
        | def ready: . == "ready" or . == "validated" or . == "completed" or . == "passed";
        def surface($id): first($root.surfaces[]? | select(.id == $id));
        def routes_checked($id):
          ((surface($id).routes // []) | length) > 0
          and all((surface($id).routes // [])[];
            (.method // "") != ""
            and (.path // "" | startswith("/api/"))
            and ((.status // 0) >= 200 and (.status // 0) < 300)
            and (.schema_checked == true)
          );
        all(["admin-console", "operator-console", "builder-console", "ops-console"][];
          (surface(.).status // "unknown" | ready)
          and (surface(.).live_api_readback == true)
          and (surface(.).authorization_boundaries_checked == true)
          and (surface(.).no_fake_completion_state == true)
          and routes_checked(.)
        )
        and (.live_api_truth.status // "unknown" | ready)
        and (.live_api_truth.route_coverage_tested == true)
        and (.live_api_truth.live_endpoint_coverage_tested == true)
        and (.live_api_truth.backend_authorization_checked == true)
        and (.live_api_truth.unauthenticated_rejected == true)
        and (.live_api_truth.forbidden_role_rejected == true)
        and (.live_api_truth.fake_completion_scan_passed == true)
        and (.live_api_truth.stale_or_mock_data_scan_passed == true)
      ' "$path" >/dev/null || {
        printf '%s lacks live API truth or per-surface route evidence' "$relative_path"
        return 0
      }
      ;;
    production-deployment-safety/summary.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.production-deployment-safety-gate" PRODUCTION_DEPLOYMENT_SAFETY_EVIDENCE_FILE="$path" scripts/production-deployment-safety-gate.sh 2>&1)"; then
        printf '%s rejected by production deployment safety gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    api-enterprise-product-readiness.json|enterprise-product-readiness-gate/api-enterprise-product-readiness.json)
      jq -e '
        (.status // "") == "enterprise_product_complete"
        and (.completion_blocked == false)
        and (.required_evidence_class // "") == "customer_grade"
        and (.evidence_archive.immutable == true)
        and ((.evidence_archive.support_owner // "") | length > 0)
        and ((.evidence_archive.uri // "") | test("^(s3|gs|az|https)://"))
        and ((.evidence_archive.uri // "") | test("(^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$)") | not)
        and ((.evidence_archive.digest // "") | test("^(sha256:)?[A-Fa-f0-9]{64}$"))
        and ((.evidence_archive.retention_policy // "") | length > 0)
        and ((.lane_count // 0) == 9)
        and ((.ready_lane_count // 0) == (.lane_count // -1))
        and ((.blocked_lane_count // 1) == 0)
        and ((.lanes // []) | length == 9)
        and all(.lanes[]?; (.status // "") == "ready" and (.current_evidence_class // "") == "customer_grade")
      ' "$path" >/dev/null || {
        printf '%s does not prove enterprise product completion readiness' "$relative_path"
        return 0
      }
      ;;
    enterprise-product-completion-contract-gate/checklist.json)
      jq -e '
        (.source // "") == "enterprise-product-completion-contract-gate"
        and (.enterprise_product_status // "") == "enterprise_product_complete"
        and (.completion_blocked == false)
        and (.required_evidence_class // "") == "customer_grade"
        and (.archive_metadata_ready == true)
        and ((.support_owner // "") | length > 0)
        and ((.evidence_archive.immutable // false) == true)
        and ((.evidence_archive.uri // "") | test("^(s3|gs|az|https)://"))
        and ((.evidence_archive.uri // "") | test("(^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$)") | not)
        and ((.evidence_archive.digest // "") | test("^(sha256:)?[A-Fa-f0-9]{64}$"))
        and ((.evidence_archive.retention_policy // "") | length > 0)
        and ((.required_lanes // []) | length == 9)
        and ((.ready_lanes // []) | length == 9)
        and ((.lane_results // []) | length == 10)
        and ((.blocked_lanes // []) | length == 0)
        and (def lane_ready($lane; $source):
          any(.lane_results[]?;
            (.lane // "") == $lane
            and (.expected_source // "") == $source
            and (.status // "") == "ready"
            and ((.summary_path // "") | length > 0)
            and ((.issue // null) == null)
          );
          lane_ready("production-deployment-safety"; "production-deployment-safety-gate")
          and lane_ready("runtime-production"; "runtime-production-readiness-gate")
          and lane_ready("remote-computer-multinode"; "remote-computer-production-state-gate")
          and lane_ready("live-connector-production"; "live-connector-production-semantics-gate")
          and lane_ready("ontology-engine"; "ontology-engine-production-gate")
          and lane_ready("ontology-release-workflow-trigger"; "ontology-release-workflow-trigger-gate")
          and lane_ready("workflowpack-enterprise-lifecycle"; "workflowpack-enterprise-lifecycle-gate")
          and lane_ready("enterprise-security-admin"; "enterprise-security-production-controls-gate")
          and lane_ready("observability-ops"; "observability-ops-production-gate")
          and lane_ready("product-surfaces"; "product-surfaces-production-gate")
        )
      ' "$path" >/dev/null || {
        printf '%s does not prove customer-grade enterprise completion checklist and lane results' "$relative_path"
        return 0
      }
      enterprise_checklist_summary_path_issue=""
      if ! enterprise_checklist_summary_paths_exist "$root" "$path"; then
        printf '%s contains invalid lane summary path evidence: %s' "$relative_path" "$enterprise_checklist_summary_path_issue"
        return 0
      fi
      ;;
    workflowpack-enterprise-lifecycle/summary.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.workflowpack-enterprise-lifecycle-gate" WORKFLOWPACK_ENTERPRISE_LIFECYCLE_EVIDENCE_FILE="$path" scripts/workflowpack-enterprise-lifecycle-gate.sh 2>&1)"; then
        printf '%s rejected by WorkflowPack enterprise lifecycle gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    enterprise-security-production-controls/summary.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.enterprise-security-production-controls-gate" ENTERPRISE_SECURITY_CONTROLS_EVIDENCE_FILE="$path" scripts/enterprise-security-production-controls-gate.sh 2>&1)"; then
        printf '%s rejected by enterprise security production controls gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    observability-ops-production/summary.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.observability-ops-production-gate" OBSERVABILITY_OPS_EVIDENCE_FILE="$path" scripts/observability-ops-production-gate.sh 2>&1)"; then
        printf '%s rejected by observability ops production gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    runtime-production-recovery-evidence.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.runtime-production-readiness-gate" SOURCE_EVIDENCE_DIR="$root" RUNTIME_PRODUCTION_RECOVERY_EVIDENCE_FILE="$path" scripts/runtime-production-readiness-gate.sh 2>&1)"; then
        printf '%s rejected by runtime production readiness gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    ontology-engine-production/summary.json)
      local gate_output
      if ! gate_output="$(env EVIDENCE_DIR="$root/.ontology-engine-production-gate" ONTOLOGY_ENGINE_PRODUCTION_EVIDENCE_FILE="$path" scripts/ontology-engine-production-gate.sh 2>&1)"; then
        printf '%s rejected by ontology engine production gate: %s' "$relative_path" "$(printf '%s' "$gate_output" | tr '\n' ' ' | cut -c1-500)"
        return 0
      fi
      ;;
    ontology-release-workflow-trigger/summary.json)
      jq -e '
        .status == "ready"
        and (.evidence_class // .required_evidence_class // "") == "customer_grade"
        and ((.target.environment // "") == "production")
        and ((.target.id // .target.deployment_id // .target.cluster_id // "") | length > 0)
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // "") | length > 0)
        and ((.checked_at // .validated_at // .completed_at // .timestamp // "") | length > 0)
        and ((.support_owner // .workflow_owner // .oncall_owner // "") | length > 0)
        and ((.evidence_archive.immutable // .archive.immutable // false) == true)
        and ((.evidence_archive.uri // .archive.uri // "") | length > 0)
        and ((.evidence_archive.digest // .archive.digest // "") | length > 0)
        and ((.evidence_archive.retention_policy // .archive.retention_policy // "") | length > 0)
        and ((.domain_scope // "") | length > 0)
        and ((.workflow_definition_id // "") | length > 0)
        and ((.workflow_run_id // "") | length > 0)
        and ((.ontology_release_id // "") | length > 0)
        and (.checks.release_promoted == true)
        and (.checks.workflow_trigger_reported == true)
        and (.checks.workflow_run_queued == true)
        and (.checks.audit_log_recorded == true)
        and (.checks.scheduler_drain_exposed == true)
        and (.checks.readiness_reflected == true)
      ' "$path" >/dev/null || {
        printf '%s is incomplete' "$relative_path"
        return 0
      }
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

manifest_value_mismatch_issue() {
  local label="$1"
  local expected="$2"
  local actual="$3"

  if [[ -z "$expected" ]]; then
    return 0
  fi
  value_mismatch_issue "$label" "$expected" "$actual"
}

enterprise_archive_metadata_mismatch_issue() {
  local root="$1"
  local relative_path="$2"
  local checklist="$root/enterprise-product-completion-contract-gate/checklist.json"
  local artifact="$root/$relative_path"
  local expected_support_owner
  local expected_uri
  local expected_digest
  local expected_retention_policy
  local expected_immutable
  local actual_support_owner
  local actual_uri
  local actual_digest
  local actual_retention_policy
  local actual_immutable
  local issue

  if [[ ! -s "$checklist" || ! -s "$artifact" ]]; then
    return 0
  fi

  expected_support_owner="$(jq -r '.support_owner // ""' "$checklist")"
  expected_uri="$(jq -r '.evidence_archive.uri // ""' "$checklist")"
  expected_digest="$(jq -r '.evidence_archive.digest // ""' "$checklist")"
  expected_retention_policy="$(jq -r '.evidence_archive.retention_policy // ""' "$checklist")"
  expected_immutable="$(jq -r '.evidence_archive.immutable // false' "$checklist")"
  actual_support_owner="$(jq -r '.evidence_archive.support_owner // ""' "$artifact")"
  actual_uri="$(jq -r '.evidence_archive.uri // ""' "$artifact")"
  actual_digest="$(jq -r '.evidence_archive.digest // ""' "$artifact")"
  actual_retention_policy="$(jq -r '.evidence_archive.retention_policy // ""' "$artifact")"
  actual_immutable="$(jq -r '.evidence_archive.immutable // false' "$artifact")"

  for issue in \
    "$(value_mismatch_issue "$relative_path evidence_archive.support_owner" "$expected_support_owner" "$actual_support_owner")" \
    "$(value_mismatch_issue "$relative_path evidence_archive.uri" "$expected_uri" "$actual_uri")" \
    "$(value_mismatch_issue "$relative_path evidence_archive.digest" "$expected_digest" "$actual_digest")" \
    "$(value_mismatch_issue "$relative_path evidence_archive.retention_policy" "$expected_retention_policy" "$actual_retention_policy")" \
    "$(value_mismatch_issue "$relative_path evidence_archive.immutable" "$expected_immutable" "$actual_immutable")"; do
    if [[ -n "$issue" ]]; then
      printf '%s does not match enterprise completion checklist archive metadata' "$issue"
      return 0
    fi
  done
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
  local expected_worker_pool
  local expected_state_claim
  local expected_state_backend
  local worker_cluster_id
  local worker_pool
  local state_cluster_id
  local state_claim
  local state_backend
  local sidecar_cluster_id
  local summary_worker_cluster_id
  local summary_worker_pool
  local summary_state_cluster_id
  local summary_state_claim
  local summary_state_backend
  local summary_sidecar_cluster_id
  local expected_tenant_deployment_id
  local tenant_deployment_id
  local expected_policy_controller_id
  local expected_policy_store_id
  local expected_policy_deployment_id
  local policy_controller_id
  local policy_store_id
  local policy_deployment_id
  local due_run_policy_controller_id
  local due_run_policy_store_id
  local due_run_policy_deployment_id
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
  expected_worker_pool="$(jq -r '.expected_targets.worker_remote_computer.worker_pool // .expected_targets.worker_remote_computer.pool // .expected_targets.worker_remote_computer.queue // ""' "$manifest")"
  issue="$(target_identity_issue "expected worker/Remote Computer worker pool" "$expected_worker_pool")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  expected_state_claim="$(jq -r '.expected_targets.worker_remote_computer.state_claim // .expected_targets.worker_remote_computer.claim // .expected_targets.worker_remote_computer.pvc // ""' "$manifest")"
  issue="$(target_identity_issue "expected Remote Computer state claim" "$expected_state_claim")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  expected_state_backend="$(jq -r '.expected_targets.worker_remote_computer.state_backend // .expected_targets.worker_remote_computer.distributed_state_backend // .expected_targets.worker_remote_computer.storage_backend // ""' "$manifest")"
  if [[ -n "$expected_state_backend" ]]; then
    if ! is_distributed_state_backend "$expected_state_backend"; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: expected Remote Computer state backend=$expected_state_backend is not distributed" >&2
    fi
  fi
  worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/worker-load-validation-evidence.json" 2>/dev/null || echo "")"
  worker_pool="$(jq -r '.response.controller_execution.worker_pool // .response.controller_execution.pool_id // .response.controller_execution.queue // .response.controller_execution.queue_name // ""' "$root/worker-load-validation-evidence.json" 2>/dev/null || echo "")"
  state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  state_claim="$(jq -r '.response.controller_execution.state_claim // .response.controller_execution.claim // .response.controller_execution.pvc // .response.controller_execution.persistent_volume_claim // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  state_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$root/remote-computer-sidecar-recovery-evidence.json" 2>/dev/null || echo "")"
  summary_worker_cluster_id="$(jq -r '.worker.cluster_id // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_worker_pool="$(jq -r '.worker.worker_pool // .worker.pool_id // .worker.queue // .worker.queue_name // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_state_cluster_id="$(jq -r '.remote_computer.state_sync_cluster_id // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_state_claim="$(jq -r '.remote_computer.state_claim // .remote_computer.claim // .remote_computer.pvc // .remote_computer.persistent_volume_claim // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_state_backend="$(jq -r '.remote_computer.distributed_state_backend // .remote_computer.state_backend // .remote_computer.storage_backend // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  summary_sidecar_cluster_id="$(jq -r '.remote_computer.sidecar_cluster_id // ""' "$root/worker-remote-computer/summary.json" 2>/dev/null || echo "")"
  for issue in \
    "$(value_mismatch_issue "worker cluster id" "$expected_cluster_id" "$worker_cluster_id")" \
    "$(value_mismatch_issue "remote state-sync cluster id" "$expected_cluster_id" "$state_cluster_id")" \
    "$(value_mismatch_issue "remote sidecar cluster id" "$expected_cluster_id" "$sidecar_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary worker cluster id" "$expected_cluster_id" "$summary_worker_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary state-sync cluster id" "$expected_cluster_id" "$summary_state_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary sidecar cluster id" "$expected_cluster_id" "$summary_sidecar_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer summary worker evidence cluster id" "$worker_cluster_id" "$summary_worker_cluster_id")" \
    "$(value_mismatch_issue "worker/Remote Computer worker pool" "$expected_worker_pool" "$worker_pool")" \
    "$(value_mismatch_issue "worker/Remote Computer summary worker pool" "$expected_worker_pool" "$summary_worker_pool")" \
    "$(value_mismatch_issue "worker/Remote Computer summary worker pool evidence" "$worker_pool" "$summary_worker_pool")" \
    "$(value_mismatch_issue "Remote Computer state claim" "$expected_state_claim" "$state_claim")" \
    "$(value_mismatch_issue "worker/Remote Computer summary state claim" "$expected_state_claim" "$summary_state_claim")" \
    "$(value_mismatch_issue "worker/Remote Computer summary state claim evidence" "$state_claim" "$summary_state_claim")" \
    "$(manifest_value_mismatch_issue "Remote Computer state backend" "$expected_state_backend" "$state_backend")" \
    "$(manifest_value_mismatch_issue "worker/Remote Computer summary state backend" "$expected_state_backend" "$summary_state_backend")" \
    "$(value_mismatch_issue "worker/Remote Computer summary state backend evidence" "$state_backend" "$summary_state_backend")" \
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
  expected_policy_store_id="$(jq -r '.expected_targets.policy_rollout.policy_store_id // .expected_targets.policy_rollout.store_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected policy rollout policy_store_id" "$expected_policy_store_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  expected_policy_deployment_id="$(jq -r '.expected_targets.policy_rollout.deployment_id // .expected_targets.policy_rollout.policy_deployment_id // ""' "$manifest")"
  issue="$(target_identity_issue "expected policy rollout deployment_id" "$expected_policy_deployment_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  policy_controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$root/policy-rollout-orchestration-validation-evidence.json" 2>/dev/null || echo "")"
  policy_store_id="$(jq -r '.response.controller_execution.policy_store_id // ""' "$root/policy-rollout-orchestration-validation-evidence.json" 2>/dev/null || echo "")"
  policy_deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$root/policy-rollout-orchestration-validation-evidence.json" 2>/dev/null || echo "")"
  due_run_policy_controller_id="$(jq -r '.response.controller_id // ""' "$root/policy-rollout-due-run-evidence.json" 2>/dev/null || echo "")"
  due_run_policy_store_id="$(jq -r '.response.policy_store_id // ""' "$root/policy-rollout-due-run-evidence.json" 2>/dev/null || echo "")"
  due_run_policy_deployment_id="$(jq -r '.response.deployment_id // ""' "$root/policy-rollout-due-run-evidence.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "policy rollout controller id" "$expected_policy_controller_id" "$policy_controller_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi
  for issue in \
    "$(value_mismatch_issue "policy rollout policy_store_id" "$expected_policy_store_id" "$policy_store_id")" \
    "$(value_mismatch_issue "policy rollout deployment_id" "$expected_policy_deployment_id" "$policy_deployment_id")" \
    "$(value_mismatch_issue "policy due-run controller id" "$expected_policy_controller_id" "$due_run_policy_controller_id")" \
    "$(value_mismatch_issue "policy due-run policy_store_id" "$expected_policy_store_id" "$due_run_policy_store_id")" \
    "$(value_mismatch_issue "policy due-run deployment_id" "$expected_policy_deployment_id" "$due_run_policy_deployment_id")"; do
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

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
    stage2-production-evidence-preflight.json
    production-evidence-run.json
    worker-load-validation-evidence.json
    remote-computer-state-sync-evidence.json
    remote-computer-sidecar-recovery-evidence.json
    worker-remote-computer/summary.json
    remote-computer-session-pod-lifecycle-evidence.json
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
    production-deployment-safety/summary.json
    product-surfaces/summary.json
    workflowpack-enterprise-lifecycle/summary.json
    enterprise-security-production-controls/summary.json
    observability-ops-production/summary.json
    live-connector-production-semantics/tmall-top/summary.json
    live-connector-production-semantics/taobao-open-platform/summary.json
    live-connector-production-semantics/xiaohongshu-shop/summary.json
    live-connector-production-semantics/xianyu-goofish/summary.json
    live-connector-production-semantics/tiktok-shop-open-api/summary.json
    live-connector-production-semantics/amazon-selling-partner-api/summary.json
    live-connector-production-semantics/github-connector/summary.json
    live-connector-production-semantics/lark-mcp/summary.json
    live-connector-production-semantics/feishu-mcp/summary.json
    live-connector-production-semantics/lark-native/summary.json
    live-connector-production-semantics/feishu-native/summary.json
    runtime-production-recovery-evidence.json
    ontology-engine-production/summary.json
    ontology-release-workflow-trigger/summary.json
    api-enterprise-product-readiness.json
    enterprise-product-readiness-gate/api-enterprise-product-readiness.json
    enterprise-product-completion-contract-gate/checklist.json
  )

  for artifact in "${required_artifacts[@]}"; do
    issue="$(artifact_issue "$root" "$artifact")"
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

  for artifact in \
    api-enterprise-product-readiness.json \
    enterprise-product-readiness-gate/api-enterprise-product-readiness.json; do
    issue="$(enterprise_archive_metadata_mismatch_issue "$root" "$artifact")"
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

  local live_connector_gate_output
  if ! live_connector_gate_output="$(env EVIDENCE_DIR="$root/.live-connector-production-semantics-gate" SOURCE_EVIDENCE_DIR="$root/live-connector-production-semantics" scripts/live-connector-production-semantics-gate.sh 2>&1)"; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: live connector production semantics rejected by gate: $(printf '%s' "$live_connector_gate_output" | tr '\n' ' ' | cut -c1-500)" >&2
  fi

  worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/worker-load-validation-evidence.json" 2>/dev/null || echo "")"
  state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$root/remote-computer-sidecar-recovery-evidence.json" 2>/dev/null || echo "")"
  if [[ -n "$worker_cluster_id" && -n "$state_cluster_id" && -n "$sidecar_cluster_id" ]]; then
    if ! [[ "$worker_cluster_id" == "$state_cluster_id" && "$worker_cluster_id" == "$sidecar_cluster_id" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: worker, state-sync, and sidecar evidence do not share one cluster id" >&2
    fi
  fi

  if verify_run_manifest "$root"; then
    issue=0
  else
    issue=$?
  fi
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
  local manifest_verification_required
  local manifest_verification_status
  local manifest_verifier
  local manifest_verify_flag
  local manifest_break_glass
  local manifest_customer_grade
  local allow_legacy_manifest
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

  allow_legacy_manifest=0
  if [[ "${ALLOW_LEGACY_STAGE2_ARCHIVE_MANIFEST:-0}" == "1" && "${MANDOFORGE_STAGE2_ARCHIVE_SELF_TEST:-0}" == "1" ]]; then
    allow_legacy_manifest=1
  fi

  if [[ "$allow_legacy_manifest" != "1" ]]; then
    manifest_verification_required="$(grep -E '^verification_required=' "$manifest_file" | sed 's/^verification_required=//' || true)"
    manifest_verification_status="$(grep -E '^verification_status=' "$manifest_file" | sed 's/^verification_status=//' || true)"
    manifest_verifier="$(grep -E '^verifier=' "$manifest_file" | sed 's/^verifier=//' || true)"
    manifest_verify_flag="$(grep -E '^verify_stage2_evidence_archive=' "$manifest_file" | sed 's/^verify_stage2_evidence_archive=//' || true)"
    manifest_break_glass="$(grep -E '^break_glass_unverified=' "$manifest_file" | sed 's/^break_glass_unverified=//' || true)"
    manifest_customer_grade="$(grep -E '^customer_grade_evidence=' "$manifest_file" | sed 's/^customer_grade_evidence=//' || true)"

    if [[ "$manifest_verification_required" != "true" || "$manifest_verifier" != "scripts/verify-stage2-evidence-archive.sh" || "$manifest_verify_flag" != "1" ]]; then
      echo "Stage 2 evidence archive manifest does not prove mandatory verifier execution" >&2
      exit 1
    fi

    if [[ "${ALLOW_PENDING_STAGE2_ARCHIVE_MANIFEST:-0}" == "1" ]]; then
      if [[ "$manifest_verification_status" != "pending" && "$manifest_verification_status" != "passed" ]]; then
        echo "Stage 2 evidence archive manifest has invalid pre-verification status: $manifest_verification_status" >&2
        exit 1
      fi
    elif [[ "$manifest_verification_status" != "passed" ]]; then
      echo "Stage 2 evidence archive manifest does not record verification_status=passed" >&2
      exit 1
    fi

    if [[ "$manifest_break_glass" != "0" ]]; then
      echo "Stage 2 evidence archive manifest was produced with break-glass verification disabled" >&2
      exit 1
    fi

    if [[ "${ALLOW_PENDING_STAGE2_ARCHIVE_MANIFEST:-0}" != "1" && "$manifest_customer_grade" != "true" ]]; then
      echo "Stage 2 evidence archive manifest does not mark the archive as customer-grade evidence" >&2
      exit 1
    fi
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
  cat >"$tmpdir/evidence/stage2-production-evidence-preflight.json" <<'JSON'
{
  "source": "stage2-production-evidence-preflight",
  "status": "passed",
  "generated_at": "1970-01-01T00:00:00Z",
  "env_file": "/evidence/stage2-production-controller.env",
  "pass_count": 4,
  "fail_count": 0,
  "checks": [
    {"status": "passed", "scope": "global", "detail": "global: RUN_STAGE2_PRODUCTION_VALIDATIONS is enabled"},
    {"status": "passed", "scope": "evidence-archive", "detail": "evidence-archive: MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_URI points at immutable production evidence storage"},
    {"status": "passed", "scope": "worker-remote-computer", "detail": "worker-remote-computer: MANDOFORGE_STAGE2_REMOTE_STATE_BACKEND names a supported distributed state backend"},
    {"status": "passed", "scope": "managed-session-runtime", "detail": "managed-session-runtime: RUN_STAGE2_MANAGED_SESSION_RESTART_RESUME is enabled"}
  ]
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
      "worker_pool": "managed-agents-prod",
      "load_validated": true,
      "isolated_worker_pool_configured": true,
      "load_checks": [
        {"cluster_id": "prod-cluster-1", "name": "queue-depth-load-validation", "worker_pool": "managed-agents-prod", "status": "passed", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state",
      "state_backend": "juicefs"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1",
      "rls_tables": ["public.sessions", "public.session_events"]
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
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
      "checked_path_count": 6,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "state_sync_node_count": 3,
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  mkdir -p "$tmpdir/evidence/worker" "$tmpdir/evidence/remote-computer"
  cat >"$tmpdir/evidence/worker/summary.txt" <<'EOF'
production_blocked_count=0
EOF
  cat >"$tmpdir/evidence/remote-computer/summary.txt" <<'EOF'
production_blocked_count=0
state_sync_cluster_id=prod-cluster-1
state_sync_node_count=3
state_sync_backend=juicefs
state_sync_state_claim=mandoforge-remote-computer-state
state_sync_checked_path_detail_count=6
sidecar_replacement_scope=cluster
sidecar_replacement_pods_healthy=true
sidecar_checked_pod_detail_count=1
EOF
  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "staged-runtime-clear", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-2", "checked_at": "1970-01-01T00:00:00Z"}
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
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 1,
    "skipped_count": 1,
    "scanned_revisions": [
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned", "audit_id": "policy-due-run-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
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

  set +e
  env -u ALLOW_LEGACY_STAGE2_ARCHIVE_MANIFEST "$0" "$archive" >/tmp/mandoforge-stage2-archive-manifest-metadata-negative.out 2>/tmp/mandoforge-stage2-archive-manifest-metadata-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing archive manifest verification metadata to fail" >&2
    exit 1
  fi
  if ! grep -q "mandatory verifier execution" /tmp/mandoforge-stage2-archive-manifest-metadata-negative.err; then
    echo "Stage 2 archive verifier self-test expected missing manifest verification metadata to fail before semantic evidence checks" >&2
    cat /tmp/mandoforge-stage2-archive-manifest-metadata-negative.err >&2
    exit 1
  fi

  set +e
  env -u MANDOFORGE_STAGE2_ARCHIVE_SELF_TEST ALLOW_LEGACY_STAGE2_ARCHIVE_MANIFEST=1 "$0" "$archive" >/tmp/mandoforge-stage2-archive-manifest-legacy-env-negative.out 2>/tmp/mandoforge-stage2-archive-manifest-legacy-env-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected legacy manifest override without self-test marker to fail" >&2
    exit 1
  fi
  if ! grep -q "mandatory verifier execution" /tmp/mandoforge-stage2-archive-manifest-legacy-env-negative.err; then
    echo "Stage 2 archive verifier self-test expected legacy manifest override without self-test marker to fail before semantic evidence checks" >&2
    cat /tmp/mandoforge-stage2-archive-manifest-legacy-env-negative.err >&2
    exit 1
  fi

  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
    echo "verification_required=true"
    echo "verification_status=passed"
    echo "verifier=scripts/verify-stage2-evidence-archive.sh"
    echo "verify_stage2_evidence_archive=1"
    echo "break_glass_unverified=0"
    echo "customer_grade_evidence=true"
  } >"${archive}.manifest.txt"
  set +e
  env -u ALLOW_LEGACY_STAGE2_ARCHIVE_MANIFEST ALLOW_BLOCKED=1 "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-target-allow-blocked.out 2>/tmp/mandoforge-stage2-archive-finance-target-allow-blocked.err
  blocked_status="$?"
  set -e
  if [[ "$blocked_status" != "0" ]]; then
    echo "Stage 2 archive verifier self-test expected ALLOW_BLOCKED=1 to tolerate semantic inventory issues" >&2
    cat /tmp/mandoforge-stage2-archive-finance-target-allow-blocked.err >&2
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
  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1",
      "rls_tables": ["public.sessions", "public.session_events"]
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
    },
    "vault_kms": {
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1"
    },
    "finance": {
      "system_id": "quickbooks-prod-2"
    },
    "managed_session_runtime": {
      "target_id": "managed-session-runtime-prod-1"
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-system-id-mismatch-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-system-id-mismatch-negative.out 2>/tmp/mandoforge-stage2-archive-finance-system-id-mismatch-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched finance ERP system id to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1",
      "rls_tables": ["public.sessions", "public.session_events"]
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
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
  cat >"$tmpdir/evidence/finance-close-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "completed",
    "actions": [],
    "close_controller_configured": true,
    "close_controller_execution": {
      "status": "closed",
      "close_id": "netsuite-close-prod-1",
      "steps": [
        {"name": "export-present", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "accounting-period-open", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-close-action-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-close-action-negative.out 2>/tmp/mandoforge-stage2-archive-finance-close-action-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing finance close controller action evidence to fail" >&2
    exit 1
  fi

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
        {"name": "export-present", "status": "failed"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-close-step-status-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-close-step-status-negative.out 2>/tmp/mandoforge-stage2-archive-finance-close-step-status-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected invalid finance close step status evidence to fail" >&2
    exit 1
  fi

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
        {"name": "export-present", "status": "passed"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-close-step-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-close-step-audit-negative.out 2>/tmp/mandoforge-stage2-archive-finance-close-step-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing finance close step audit evidence to fail" >&2
    exit 1
  fi

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
        {"name": "export-present", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "export-present", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-close-step-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-close-step-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-finance-close-step-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate finance close step evidence to fail" >&2
    exit 1
  fi

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
        {"name": "export-present", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "accounting-period-open", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
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
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"}
    ],
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
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "verify-secret-consumers", "status": "passed", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
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
        {"name": "export-present", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "accounting-period-open", "status": "passed", "close_id": "netsuite-close-prod-1", "audit_id": "finance-close-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
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
      {"name": "close-evidence", "status": "passed", "reconciliation_id": "netsuite-reconciliation-prod-1", "audit_id": "finance-reconciliation-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"name": "export-recent", "status": "passed", "reconciliation_id": "netsuite-reconciliation-prod-1", "audit_id": "finance-reconciliation-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  cat >"$tmpdir/evidence/finance-reconciliation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "reconciled",
    "reconciliation_id": "netsuite-reconciliation-prod-1",
    "checks": []
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-reconciliation-checks-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-reconciliation-checks-negative.out 2>/tmp/mandoforge-stage2-archive-finance-reconciliation-checks-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing finance reconciliation check evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-reconciliation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "reconciled",
    "reconciliation_id": "netsuite-reconciliation-prod-1",
    "checks": [
      {"name": "close-evidence", "status": "failed"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-reconciliation-check-status-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-reconciliation-check-status-negative.out 2>/tmp/mandoforge-stage2-archive-finance-reconciliation-check-status-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected invalid finance reconciliation check status evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-reconciliation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "reconciled",
    "reconciliation_id": "netsuite-reconciliation-prod-1",
    "checks": [
      {"name": "close-evidence", "status": "passed"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-reconciliation-check-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-reconciliation-check-audit-negative.out 2>/tmp/mandoforge-stage2-archive-finance-reconciliation-check-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing finance reconciliation check audit evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-reconciliation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "reconciled",
    "reconciliation_id": "netsuite-reconciliation-prod-1",
    "checks": [
      {"name": "close-evidence", "status": "passed", "reconciliation_id": "netsuite-reconciliation-prod-1", "audit_id": "finance-reconciliation-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"name": "close-evidence", "status": "passed", "reconciliation_id": "netsuite-reconciliation-prod-1", "audit_id": "finance-reconciliation-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-reconciliation-check-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-reconciliation-check-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-finance-reconciliation-check-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate finance reconciliation check evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-reconciliation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "reconciled",
    "reconciliation_id": "netsuite-reconciliation-prod-1",
    "checks": [
      {"name": "close-evidence", "status": "passed", "reconciliation_id": "netsuite-reconciliation-prod-1", "audit_id": "finance-reconciliation-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"name": "export-recent", "status": "passed", "reconciliation_id": "netsuite-reconciliation-prod-1", "audit_id": "finance-reconciliation-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
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
    "latest_file_name": "mandoforge-usage-export.csv",
    "latest_bytes": 128,
    "delivery_count": 1,
    "delivery_receipts": [
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "netsuite-prod-1", "file_name": "mandoforge-usage-export.csv", "byte_count": 128, "status": "posted", "record_count": 1, "posted_at": "1970-01-01T00:00:00Z", "audit_id": "finance-delivery-audit-1"}
    ]
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
    "worker_drain_observed": true,
    "pending_event_seq_start": 41,
    "pending_event_seq_end": 42
  },
  "restart": {
    "api_restarted": true,
    "worker_restarted": true
  },
  "resume": {
    "session_state_resumed": true,
    "processed_event_seq_preserved": true,
    "processed_event_seq_before_restart": 42,
    "processed_event_seq_after_resume": 42
  },
  "thread_lineage": {
    "preserved": true,
    "original_thread_id": "thread-prod-1",
    "resumed_thread_id": "thread-prod-1"
  },
  "lease_fencing": {
    "finalization_fenced": true,
    "stale_worker_rejected": true,
    "active_worker_lease_id": "lease-active-prod-1",
    "stale_worker_lease_id": "lease-stale-prod-1",
    "stale_rejection_reason": "stale lease generation rejected"
  },
  "runtime_turn": {
    "completed": true,
    "final_message_preserved": true,
    "turn_id": "turn-prod-1",
    "final_message": "managed session restart resume validated"
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-session-pod-lifecycle-evidence.json" <<'JSON'
{
  "cluster_id": "prod-cluster-1",
  "session_id": "session-prod-1",
  "remote_computer_id": "remote-computer-prod-1",
  "lease_id": "lease-prod-1",
  "pod_name": "mandoforge-agent-session-prod-1",
  "pod_phase": "Running",
  "pod_labels": {
    "mandoforge.io/session-id": "session-prod-1",
    "mandoforge.io/remote-computer-id": "remote-computer-prod-1",
    "mandoforge.io/tenant-id": "tenant-prod-1",
    "mandoforge.io/lease-id": "lease-prod-1",
    "mandoforge.io/lifecycle": "assigned"
  },
  "live_create": {"ok": true},
  "approved_exec": {"ok": true},
  "heartbeat": {"observed": true},
  "lease_release": {"ok": true},
  "pod_delete": {"ok": true},
  "orphan_sweep": {"ok": true, "orphan_count": 0}
}
JSON
  cat >"$tmpdir/evidence/runtime-production-recovery-evidence.json" <<'JSON'
{
  "status": "validated",
  "target": {
    "id": "runtime-prod-1",
    "kind": "production_runtime_cluster"
  },
  "backup_restore": {
    "status": "validated",
    "preserved_resources": [
      "sessions",
      "events",
      "approvals",
      "tool_calls",
      "artifacts",
      "audit_logs",
      "workflow_runs",
      "semantic_objects",
      "context_packets"
    ],
    "audit_id": "backup-restore-audit-1",
    "completed_at": "1970-01-01T00:00:00Z"
  },
  "dead_letter_replay": {
    "status": "validated",
    "dead_letter_queue_configured": true,
    "manual_replay_tested": true,
    "audit_id": "dead-letter-audit-1",
    "completed_at": "1970-01-01T00:00:00Z"
  },
  "idempotency": {
    "status": "validated",
    "external_side_effect_idempotency_covered": true,
    "idempotency_key_count": 3
  }
}
JSON
  mkdir -p "$tmpdir/evidence/production-deployment-safety"
  cat >"$tmpdir/evidence/production-deployment-safety/summary.json" <<'JSON'
{
  "status": "ready",
  "evidence_class": "customer_grade",
  "required_evidence_class": "customer_grade",
  "source": "production-deployment-safety-gate",
  "blocked_count": 0,
  "target": {
    "id": "deployment-prod-1",
    "kind": "production_deployment",
    "environment": "production"
  },
  "audit_id": "deployment-safety-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "platform-oncall",
  "evidence_archive": {
    "uri": "s3://mandoforge-prod-evidence/deployment-safety",
    "immutable": true,
    "digest": "sha256:deployment-safety",
    "retention_policy": "seven-years"
  },
  "checks": {
    "no_example_secret_applied": true,
    "external_secret_delivery_proven": true,
    "no_default_credentials": true,
    "durable_workspace_storage": true,
    "no_insecure_auth": true,
    "provider_runtime_production": true,
    "remote_computer_kubernetes": true,
    "launch_preflight_passed": true,
    "enterprise_completion_contract_inventory_passed": true,
    "customer_data_boundary_documented": true
  }
}
JSON
  mkdir -p "$tmpdir/evidence/product-surfaces"
  cat >"$tmpdir/evidence/product-surfaces/summary.json" <<'JSON'
{
  "status": "ready",
  "evidence_class": "customer_grade",
  "required_evidence_class": "customer_grade",
  "source": "ontology-release-workflow-trigger-gate",
  "blocked_count": 0,
  "target": {
    "id": "product-surfaces-prod-1",
    "kind": "production_product_surface",
    "environment": "production",
    "base_url": "https://mandoforge-prod.mandonothing.com",
    "git_sha": "0123456789abcdef0123456789abcdef01234567",
    "image_tag": "prod-2026-06-29"
  },
  "audit_id": "product-surfaces-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "product-oncall",
  "freshness": {
    "expires_at": "2999-01-01T00:00:00Z"
  },
  "evidence_archive": {
    "uri": "s3://mandoforge-prod-evidence/product-surfaces",
    "immutable": true,
    "digest": "sha256:product-surfaces",
    "retention_policy": "seven-years"
  },
  "surfaces": [
    {
      "id": "admin-console",
      "status": "ready",
      "live_api_readback": true,
      "authorization_boundaries_checked": true,
      "no_fake_completion_state": true,
      "features": ["tenants", "teams", "agents", "runtime_profiles", "providers", "policies", "approvals", "connectors", "budgets", "release_state"],
      "routes": [{"method": "GET", "path": "/api/enterprise-product/readiness", "status": 200, "schema_checked": true}]
    },
    {
      "id": "operator-console",
      "status": "ready",
      "live_api_readback": true,
      "authorization_boundaries_checked": true,
      "no_fake_completion_state": true,
      "features": ["blocked_work", "approvals", "runs", "replay", "artifacts", "execution_jobs", "session_loop_jobs", "manual_repair"],
      "routes": [{"method": "GET", "path": "/api/workflow-runs", "status": 200, "schema_checked": true}]
    },
    {
      "id": "builder-console",
      "status": "ready",
      "live_api_readback": true,
      "authorization_boundaries_checked": true,
      "no_fake_completion_state": true,
      "features": ["workflow_packs", "ontology_builder", "connector_mapping", "eval_gates", "release_gates"],
      "routes": [{"method": "GET", "path": "/api/ontology/engine-readiness", "status": 200, "schema_checked": true}]
    },
    {
      "id": "ops-console",
      "status": "ready",
      "live_api_readback": true,
      "authorization_boundaries_checked": true,
      "no_fake_completion_state": true,
      "features": ["health", "workers", "queues", "costs", "alerts", "deployments", "incident_evidence"],
      "routes": [{"method": "GET", "path": "/api/observability", "status": 200, "schema_checked": true}]
    }
  ],
  "live_api_truth": {
    "status": "ready",
    "route_coverage_tested": true,
    "live_endpoint_coverage_tested": true,
    "backend_authorization_checked": true,
    "unauthenticated_rejected": true,
    "forbidden_role_rejected": true,
    "fake_completion_scan_passed": true,
    "stale_or_mock_data_scan_passed": true
  }
}
JSON
  mkdir -p "$tmpdir/evidence/workflowpack-enterprise-lifecycle"
  cat >"$tmpdir/evidence/workflowpack-enterprise-lifecycle/summary.json" <<'JSON'
{
  "status": "ready",
  "evidence_class": "customer_grade",
  "target": {
    "id": "workflowpack-prod-1",
    "kind": "workflowpack_lifecycle",
    "environment": "production"
  },
  "pack": {
    "id": "commerce-pack",
    "version": "2026.06.29"
  },
  "audit_id": "workflowpack-lifecycle-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "builder-oncall",
  "evidence_archive": {
    "uri": "s3://mandoforge-prod-evidence/workflowpack-lifecycle",
    "immutable": true,
    "digest": "sha256:workflowpack-lifecycle",
    "retention_policy": "seven-years"
  },
  "checks": {
    "install_audited": true,
    "stage_audited": true,
    "release_promoted": true,
    "rollback_verified": true,
    "archive_verified": true,
    "onboarding_profiles_complete": true,
    "connector_quality_passed": true,
    "eval_regression_passed": true,
    "canary_promoted": true,
    "compatibility_matrix_passed": true,
    "tenant_overrides_policy_enforced": true,
    "managed_workflow_recovery_passed": true
  },
  "compatibility_matrix": {
    "versions": ["2026.06.28", "2026.06.29"]
  },
  "tenant_overrides": {
    "validated_tenants": ["tenant-prod-1"]
  },
  "managed_workflow_runtime": {
    "recovery_checks": ["scheduler_retry", "fan_in_completion", "durable_transitions", "expired_lease_reclaim"]
  }
}
JSON
  mkdir -p "$tmpdir/evidence/enterprise-security-production-controls"
  cat >"$tmpdir/evidence/enterprise-security-production-controls/summary.json" <<'JSON'
{
  "status": "ready",
  "target": {
    "id": "security-prod-1",
    "kind": "production_security_controls"
  },
  "audit_id": "security-controls-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "security-oncall",
  "controls": [
    {"id": "identity-provisioning", "status": "ready", "sso_protocol": "oidc", "scim_enabled": true, "directory_id": "directory-prod-1"},
    {"id": "tenant-rls-abac", "status": "ready", "rls_forced": true, "abac_tested": true},
    {"id": "vault-kms-rotation-recovery", "status": "ready", "production_kms_backend": true, "rotation_tested": true, "recovery_tested": true},
    {"id": "approval-break-glass", "status": "ready", "break_glass_tested": true, "audit_captured": true},
    {"id": "audit-export-siem", "status": "ready", "delivery_tested": true, "correlation_fields": ["tenant_id", "session_id", "tool_call_id"]},
    {"id": "data-governance", "status": "ready", "retention_tested": true, "legal_hold_tested": true, "export_tested": true, "deletion_tested": true, "pii_redaction_tested": true, "dlp_tested": true},
    {"id": "security-incident-operations", "status": "ready", "runbook_rehearsed": true, "evidence_archive_immutable": true}
  ]
}
JSON
  mkdir -p "$tmpdir/evidence/observability-ops-production"
  cat >"$tmpdir/evidence/observability-ops-production/summary.json" <<'JSON'
{
  "status": "ready",
  "target": {
    "id": "observability-prod-1",
    "kind": "production_observability"
  },
  "audit_id": "observability-ops-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "ops-oncall",
  "evidence_archive": {
    "uri": "s3://mandoforge-prod-evidence/observability",
    "immutable": true
  },
  "correlation": {"status": "ready", "fields": ["tenant_id", "session_id", "workflow_run_id", "tool_call_id", "worker_id", "connector_id", "provider_id"]},
  "alerts": {"status": "ready", "delivery_tested": true, "coverage": ["failed_jobs", "stale_leases", "delivery_failures", "connector_degradation", "provider_degradation", "budget_breach", "queue_backlog"]},
  "versions": {"status": "ready", "visible": ["deployment", "migration", "workflow_pack", "ontology", "connector"]},
  "incident_timeline": {"status": "ready", "audit_captured": true},
  "manual_repair": {"status": "ready", "actions_audited": true, "replay_tested": true},
  "slos": {"status": "ready", "coverage": ["runtime", "connector", "worker", "approval", "remote_computer"]},
  "runbooks": {"status": "ready", "rehearsed": true, "owner_acknowledged": true}
}
JSON
  mkdir -p "$tmpdir/evidence/ontology-release-workflow-trigger"
  mkdir -p "$tmpdir/evidence/ontology-engine-production"
  cat >"$tmpdir/evidence/ontology-engine-production/summary.json" <<'JSON'
{
  "status": "ready",
  "evidence_class": "customer_grade",
  "target": {
    "id": "ontology-engine-prod-1",
    "kind": "production_ontology_engine",
    "environment": "production"
  },
  "registry_version": "ontology-registry-2026.06.29",
  "registry": {
    "status": "ready",
    "version": "ontology-registry-2026.06.29",
    "core_version": "core-2026.06.29",
    "domain_versions": ["commerce-2026.06.29"]
  },
  "audit_id": "ontology-engine-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "ontology-oncall",
  "evidence_archive": {
    "uri": "s3://mandoforge-prod-evidence/ontology-engine",
    "immutable": true,
    "digest": "sha256:ontology-engine",
    "retention_policy": "seven-years"
  },
  "migrations": {
    "status": "ready",
    "promote_tested": true,
    "rollback_tested": true,
    "forward_migration_tested": true,
    "migration_policy_present": true
  },
  "relation_constraints": {
    "status": "ready",
    "enforced_before_policy": true,
    "coverage": ["object_type", "link_type", "cardinality", "domain_scope"]
  },
  "conflict_trust": {
    "status": "ready",
    "conflict_policy_tested": true,
    "contradiction_blocks_high_risk": true,
    "stale_fact_blocks_high_risk": true,
    "trust_downgrade_blocks_high_risk": true
  },
  "builder_approvals": {
    "status": "ready",
    "reviewable_proposals": true,
    "approved_changes_audited": true
  },
  "context_packets": {
    "status": "ready",
    "source_refs_rendered": true,
    "ontology_version_rendered": true,
    "relation_expansion_rendered": true,
    "trust_freshness_gates_enforced": true
  },
  "runtime_enforcement": {
    "status": "ready",
    "policy_precheck_uses_constraints": true
  }
}
JSON
  cat >"$tmpdir/evidence/ontology-release-workflow-trigger/summary.json" <<'JSON'
{
  "status": "ready",
  "evidence_class": "customer_grade",
  "required_evidence_class": "customer_grade",
  "source": "ontology-release-workflow-trigger-gate",
  "blocked_count": 0,
  "target": {
    "id": "ontology-trigger-prod-1",
    "kind": "production_ontology_workflow_trigger",
    "environment": "production"
  },
  "audit_id": "ontology-trigger-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "support_owner": "ontology-oncall",
  "evidence_archive": {
    "uri": "s3://mandoforge-prod-evidence/ontology-trigger",
    "immutable": true,
    "digest": "sha256:ontology-trigger",
    "retention_policy": "seven-years"
  },
  "domain_scope": "commerce",
  "workflow_definition_id": "workflow-definition-prod-1",
  "workflow_run_id": "workflow-run-prod-1",
  "ontology_release_id": "ontology-release-prod-1",
  "checks": {
    "release_promoted": true,
    "workflow_trigger_reported": true,
    "workflow_run_queued": true,
    "audit_log_recorded": true,
    "scheduler_drain_exposed": true,
    "readiness_reflected": true
  }
}
JSON
  mkdir -p "$tmpdir/evidence/enterprise-product-completion-contract-gate" "$tmpdir/evidence/enterprise-product-readiness-gate"
  mkdir -p \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/production-deployment-safety" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/runtime-production" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/remote-computer-multinode" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/live-connector-production" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/ontology-engine" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/workflowpack-enterprise-lifecycle" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/enterprise-security-admin" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/observability-ops" \
    "$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/product-surfaces"
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/production-deployment-safety/summary.json" <<'JSON'
{"source":"production-deployment-safety-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"production_deployment_safety_evidence_file":"production-deployment-safety/summary.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/runtime-production/summary.json" <<'JSON'
{"source":"runtime-production-readiness-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"runtime_recovery_status":"ready","runtime_recovery_evidence_file":"runtime-production-recovery-evidence.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/remote-computer-multinode/summary.json" <<'JSON'
{"source":"remote-computer-production-state-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"remote_evidence_dir":"remote-computer","combined_evidence_dir":"worker-remote-computer","lifecycle_evidence_file":"remote-computer-session-pod-lifecycle-evidence.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/live-connector-production/summary.json" <<'JSON'
{"source":"live-connector-production-semantics-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"connector_count":11,"source_evidence_dir":"live-connector-production-semantics"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/ontology-engine/summary.json" <<'JSON'
{"source":"ontology-engine-production-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"ontology_engine_evidence_file":"ontology-engine-production/summary.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/workflowpack-enterprise-lifecycle/summary.json" <<'JSON'
{"source":"workflowpack-enterprise-lifecycle-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"workflowpack_enterprise_lifecycle_evidence_file":"workflowpack-enterprise-lifecycle/summary.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/enterprise-security-admin/summary.json" <<'JSON'
{"source":"enterprise-security-production-controls-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"controls_evidence_file":"enterprise-security-production-controls/summary.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/observability-ops/summary.json" <<'JSON'
{"source":"observability-ops-production-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"ops_evidence_file":"observability-ops-production/summary.json"}
JSON
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/lane-gates/product-surfaces/summary.json" <<'JSON'
{"source":"product-surfaces-production-gate","required_evidence_class":"customer_grade","status":"ready","blocked_count":0,"product_surfaces_evidence_file":"product-surfaces/summary.json"}
JSON
  cat >"$tmpdir/evidence/api-enterprise-product-readiness.json" <<'JSON'
{
  "status": "enterprise_product_complete",
  "completion_blocked": false,
  "required_evidence_class": "customer_grade",
  "evidence_archive": {
    "support_owner": "platform-oncall",
    "uri": "s3://mandoforge-prod-evidence/enterprise-completion/stage2-evidence.tar.gz",
    "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "retention_policy": "p1y",
    "immutable": true
  },
  "lane_count": 9,
  "ready_lane_count": 9,
  "blocked_lane_count": 0,
  "lanes": [
    {"id": "production-deployment-safety", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "runtime-production", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "remote-computer-multinode", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "live-connector-production", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "ontology-engine", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "workflowpack-enterprise-lifecycle", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "enterprise-security-admin", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "observability-ops", "status": "ready", "current_evidence_class": "customer_grade"},
    {"id": "product-surfaces", "status": "ready", "current_evidence_class": "customer_grade"}
  ]
}
JSON
  cp "$tmpdir/evidence/api-enterprise-product-readiness.json" "$tmpdir/evidence/enterprise-product-readiness-gate/api-enterprise-product-readiness.json"
  cat >"$tmpdir/evidence/enterprise-product-completion-contract-gate/checklist.json" <<'JSON'
{
  "source": "enterprise-product-completion-contract-gate",
  "enterprise_product_status": "enterprise_product_complete",
  "completion_blocked": false,
  "required_evidence_class": "customer_grade",
  "required_lanes": [
    "production-deployment-safety",
    "runtime-production",
    "remote-computer-multinode",
    "live-connector-production",
    "ontology-engine",
    "workflowpack-enterprise-lifecycle",
    "enterprise-security-admin",
    "observability-ops",
    "product-surfaces"
  ],
  "ready_lanes": [
    "production-deployment-safety",
    "runtime-production",
    "remote-computer-multinode",
    "live-connector-production",
    "ontology-engine",
    "workflowpack-enterprise-lifecycle",
    "enterprise-security-admin",
    "observability-ops",
    "product-surfaces"
  ],
  "blocked_lanes": [],
  "lane_results": [
    {"lane": "production-deployment-safety", "expected_source": "production-deployment-safety-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/production-deployment-safety/summary.json", "issue": null},
    {"lane": "runtime-production", "expected_source": "runtime-production-readiness-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/runtime-production/summary.json", "issue": null},
    {"lane": "remote-computer-multinode", "expected_source": "remote-computer-production-state-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/remote-computer-multinode/summary.json", "issue": null},
    {"lane": "live-connector-production", "expected_source": "live-connector-production-semantics-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/live-connector-production/summary.json", "issue": null},
    {"lane": "ontology-engine", "expected_source": "ontology-engine-production-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/ontology-engine/summary.json", "issue": null},
    {"lane": "ontology-release-workflow-trigger", "expected_source": "ontology-release-workflow-trigger-gate", "status": "ready", "summary_path": "ontology-release-workflow-trigger/summary.json", "issue": null},
    {"lane": "workflowpack-enterprise-lifecycle", "expected_source": "workflowpack-enterprise-lifecycle-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/workflowpack-enterprise-lifecycle/summary.json", "issue": null},
    {"lane": "enterprise-security-admin", "expected_source": "enterprise-security-production-controls-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/enterprise-security-admin/summary.json", "issue": null},
    {"lane": "observability-ops", "expected_source": "observability-ops-production-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/observability-ops/summary.json", "issue": null},
    {"lane": "product-surfaces", "expected_source": "product-surfaces-production-gate", "status": "ready", "summary_path": "enterprise-product-completion-contract-gate/lane-gates/product-surfaces/summary.json", "issue": null}
  ]
}
JSON
  for connector_id in \
    tmall-top \
    taobao-open-platform \
    xiaohongshu-shop \
    xianyu-goofish \
    tiktok-shop-open-api \
    amazon-selling-partner-api \
    github-connector \
    lark-mcp \
    feishu-mcp \
    lark-native \
    feishu-native; do
    mkdir -p "$tmpdir/evidence/live-connector-production-semantics/$connector_id"
    cat >"$tmpdir/evidence/live-connector-production-semantics/$connector_id/summary.json" <<JSON
{
  "status": "ready",
  "connector": {
    "id": "$connector_id",
    "version": "2026.06.29"
  },
  "target": {
    "id": "$connector_id-prod-1",
    "kind": "production_connector"
  },
  "audit_id": "$connector_id-audit-1",
  "checked_at": "1970-01-01T00:00:00Z",
  "deployment_archive": {
    "uri": "s3://mandoforge-prod-evidence/connectors/$connector_id",
    "logs_uri": "s3://mandoforge-prod-evidence/connectors/$connector_id/logs",
    "support_owner": "connector-oncall",
    "immutable": true
  },
  "sandbox_live_separation": {"status": "ready"},
  "token_lifecycle": {"status": "ready", "refresh_tested": true, "expiry_tested": true, "rotation_tested": true},
  "rate_limit_retry": {"status": "ready", "error_taxonomy": ["rate_limited", "auth_expired"]},
  "idempotency_reconciliation": {"status": "ready", "idempotency_key_count": 1, "external_reconciliation_tested": true},
  "webhook_ingestion": {"status": "ready", "provenance_captured": true},
  "compensation": {"status": "ready", "mode": "compensation_or_explicit_non_compensable"},
  "approval_commit_boundary": {"status": "ready"},
  "secret_redaction": {"status": "ready", "no_raw_secret_leakage": true}
}
JSON
  done
  archive="$tmpdir/stage2-evidence-enterprise-archive-metadata-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-enterprise-archive-metadata-negative.out 2>/tmp/mandoforge-stage2-archive-enterprise-archive-metadata-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected enterprise completion checklist without archive metadata to fail" >&2
    exit 1
  fi
  if ! grep -q "customer-grade enterprise completion checklist" /tmp/mandoforge-stage2-archive-enterprise-archive-metadata-negative.err; then
    echo "Stage 2 archive verifier self-test expected missing enterprise archive metadata to fail at completion checklist validation" >&2
    cat /tmp/mandoforge-stage2-archive-enterprise-archive-metadata-negative.err >&2
    exit 1
  fi

  jq '. + {
    "support_owner": "platform-oncall",
    "archive_metadata_ready": true,
    "evidence_archive": {
      "uri": "s3://mandoforge-prod-evidence/enterprise-completion/stage2-evidence.tar.gz",
      "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "retention_policy": "p1y",
      "immutable": true
    }
  }' "$tmpdir/evidence/enterprise-product-completion-contract-gate/checklist.json" >"$tmpdir/evidence/enterprise-product-completion-contract-gate/checklist.json.tmp"
  mv "$tmpdir/evidence/enterprise-product-completion-contract-gate/checklist.json.tmp" "$tmpdir/evidence/enterprise-product-completion-contract-gate/checklist.json"
  cp "$tmpdir/evidence/api-enterprise-product-readiness.json" "$tmpdir/evidence/api-enterprise-product-readiness.json.good"
  jq '.evidence_archive.uri = "s3://mandoforge-prod-evidence/enterprise-completion/different-stage2-evidence.tar.gz"' \
    "$tmpdir/evidence/api-enterprise-product-readiness.json" >"$tmpdir/evidence/api-enterprise-product-readiness.json.tmp"
  mv "$tmpdir/evidence/api-enterprise-product-readiness.json.tmp" "$tmpdir/evidence/api-enterprise-product-readiness.json"
  cp "$tmpdir/evidence/api-enterprise-product-readiness.json" "$tmpdir/evidence/enterprise-product-readiness-gate/api-enterprise-product-readiness.json"
  archive="$tmpdir/stage2-evidence-enterprise-archive-readback-mismatch-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-enterprise-archive-readback-mismatch-negative.out 2>/tmp/mandoforge-stage2-archive-enterprise-archive-readback-mismatch-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched API archive readback metadata to fail" >&2
    exit 1
  fi
  if ! grep -q "does not match enterprise completion checklist archive metadata" /tmp/mandoforge-stage2-archive-enterprise-archive-readback-mismatch-negative.err; then
    echo "Stage 2 archive verifier self-test expected API archive readback mismatch to fail at archive metadata binding validation" >&2
    cat /tmp/mandoforge-stage2-archive-enterprise-archive-readback-mismatch-negative.err >&2
    exit 1
  fi
  mv "$tmpdir/evidence/api-enterprise-product-readiness.json.good" "$tmpdir/evidence/api-enterprise-product-readiness.json"
  cp "$tmpdir/evidence/api-enterprise-product-readiness.json" "$tmpdir/evidence/enterprise-product-readiness-gate/api-enterprise-product-readiness.json"
  cp "$tmpdir/evidence/stage2-production-evidence-preflight.json" "$tmpdir/evidence/stage2-production-evidence-preflight.json.good"
  jq '.status = "failed" | .fail_count = 1 | .checks += [{"status":"failed","scope":"global","detail":"global: RUN_STAGE2_PRODUCTION_VALIDATIONS must be true/1"}]' \
    "$tmpdir/evidence/stage2-production-evidence-preflight.json" >"$tmpdir/evidence/stage2-production-evidence-preflight.json.tmp"
  mv "$tmpdir/evidence/stage2-production-evidence-preflight.json.tmp" "$tmpdir/evidence/stage2-production-evidence-preflight.json"
  archive="$tmpdir/stage2-evidence-preflight-failed-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-preflight-failed-negative.out 2>/tmp/mandoforge-stage2-archive-preflight-failed-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected failed production evidence preflight summary to fail" >&2
    exit 1
  fi
  if ! grep -q "strict Stage 2 production evidence preflight success" /tmp/mandoforge-stage2-archive-preflight-failed-negative.err; then
    echo "Stage 2 archive verifier self-test expected failed preflight summary to fail at preflight validation" >&2
    cat /tmp/mandoforge-stage2-archive-preflight-failed-negative.err >&2
    exit 1
  fi
  mv "$tmpdir/evidence/stage2-production-evidence-preflight.json.good" "$tmpdir/evidence/stage2-production-evidence-preflight.json"
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

  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 1,
      "rls_forced_table_count": 1,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-expected-rls-table-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-expected-rls-table-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-expected-rls-table-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing configured tenant RLS table evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON

  cat >"$tmpdir/evidence/finance-export-delivery-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "delivered",
    "delivered": true,
    "target_configured": true,
    "bytes": 0
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-export-bytes-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-export-bytes-negative.out 2>/tmp/mandoforge-stage2-archive-finance-export-bytes-negative.err
  local negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected zero finance export delivery byte evidence to fail" >&2
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

  cat >"$tmpdir/evidence/managed-session-restart-resume-evidence.json" <<'JSON'
{
  "status": "validated",
  "target": {
    "id": "managed-session-runtime-prod-1",
    "kind": "managed_session_runtime"
  },
  "session_loop": {
    "enqueue_event_persisted": true,
    "worker_drain_observed": true,
    "pending_event_seq_start": 41,
    "pending_event_seq_end": 42
  },
  "restart": {
    "api_restarted": true,
    "worker_restarted": true
  },
  "resume": {
    "session_state_resumed": true,
    "processed_event_seq_preserved": false
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
  archive="$tmpdir/stage2-evidence-managed-session-cursor-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-managed-session-cursor-negative.out 2>/tmp/mandoforge-stage2-archive-managed-session-cursor-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing managed-session processed cursor evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/managed-session-restart-resume-evidence.json" <<'JSON'
{
  "status": "validated",
  "target": {
    "id": "managed-session-runtime-prod-1",
    "kind": "managed_session_runtime"
  },
  "session_loop": {
    "enqueue_event_persisted": true,
    "worker_drain_observed": true,
    "pending_event_seq_start": 41,
    "pending_event_seq_end": 42
  },
  "restart": {
    "api_restarted": true,
    "worker_restarted": true
  },
  "resume": {
    "session_state_resumed": true,
    "processed_event_seq_preserved": true,
    "processed_event_seq_before_restart": 42,
    "processed_event_seq_after_resume": 43
  },
  "thread_lineage": {
    "preserved": true,
    "original_thread_id": "thread-prod-1",
    "resumed_thread_id": "thread-prod-1"
  },
  "lease_fencing": {
    "finalization_fenced": true,
    "stale_worker_rejected": true,
    "active_worker_lease_id": "lease-active-prod-1",
    "stale_worker_lease_id": "lease-stale-prod-1",
    "stale_rejection_reason": "stale lease generation rejected"
  },
  "runtime_turn": {
    "completed": true,
    "final_message_preserved": true,
    "turn_id": "turn-prod-1",
    "final_message": "managed session restart resume validated"
  }
}
JSON
  archive="$tmpdir/stage2-evidence-managed-session-cursor-drift-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-managed-session-cursor-drift-negative.out 2>/tmp/mandoforge-stage2-archive-managed-session-cursor-drift-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected managed-session processed cursor drift evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/managed-session-restart-resume-evidence.json" <<'JSON'
{
  "status": "validated",
  "target": {
    "id": "managed-session-runtime-prod-1",
    "kind": "managed_session_runtime"
  },
  "session_loop": {
    "enqueue_event_persisted": true,
    "worker_drain_observed": true,
    "pending_event_seq_start": 41,
    "pending_event_seq_end": 42
  },
  "restart": {
    "api_restarted": true,
    "worker_restarted": true
  },
  "resume": {
    "session_state_resumed": true,
    "processed_event_seq_preserved": true,
    "processed_event_seq_before_restart": 42,
    "processed_event_seq_after_resume": 42
  },
  "thread_lineage": {
    "preserved": true,
    "original_thread_id": "thread-prod-1",
    "resumed_thread_id": "thread-prod-1"
  },
  "lease_fencing": {
    "finalization_fenced": true,
    "stale_worker_rejected": true,
    "active_worker_lease_id": "lease-active-prod-1",
    "stale_worker_lease_id": "lease-stale-prod-1",
    "stale_rejection_reason": "stale lease generation rejected"
  },
  "runtime_turn": {
    "completed": true,
    "final_message_preserved": true,
    "turn_id": "turn-prod-1",
    "final_message": "managed session restart resume validated"
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
      "node_count": 1,
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
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
  negative_status="$?"
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
      "worker_pool": "managed-agents-prod",
      "load_validated": true,
      "isolated_worker_pool_configured": false,
      "load_checks": [
        {"cluster_id": "prod-cluster-1", "name": "queue-depth-load-validation", "worker_pool": "managed-agents-prod", "status": "passed", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "worker_pool": "managed-agents-prod",
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
      "worker_pool": "managed-agents-prod",
      "load_validated": true,
      "isolated_worker_pool_configured": true
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-worker-load-check-detail-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-worker-load-check-detail-negative.out 2>/tmp/mandoforge-stage2-archive-worker-load-check-detail-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing worker load check detail evidence to fail" >&2
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
      "worker_pool": "managed-agents-prod",
      "load_validated": true,
      "isolated_worker_pool_configured": true,
      "load_checks": [
        {"cluster_id": "prod-cluster-1", "name": "queue-depth-load-validation", "worker_pool": "managed-agents-prod", "status": "passed"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-worker-load-check-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-worker-load-check-audit-negative.out 2>/tmp/mandoforge-stage2-archive-worker-load-check-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing worker load check audit evidence to fail" >&2
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
      "worker_pool": "managed-agents-prod",
      "load_validated": true,
      "isolated_worker_pool_configured": true,
      "load_check_count": 2,
      "load_checks": [
        {"cluster_id": "prod-cluster-1", "name": "queue-depth-load-validation", "worker_pool": "managed-agents-prod", "status": "passed", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "name": "queue-depth-load-validation", "worker_pool": "managed-agents-prod", "status": "passed", "audit_id": "worker-load-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-worker-load-check-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-worker-load-check-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-worker-load-check-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate worker load check detail evidence to fail" >&2
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
      "worker_pool": "managed-agents-prod",
      "load_validated": true,
      "isolated_worker_pool_configured": true,
      "load_checks": [
        {"cluster_id": "prod-cluster-1", "name": "queue-depth-load-validation", "worker_pool": "managed-agents-prod", "status": "passed", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_path_count": 6,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_path_count": 6,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_path_count": 1,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-path-claim-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-path-claim-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-path-claim-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched Remote Computer checked path claim evidence to fail" >&2
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
  archive="$tmpdir/stage2-evidence-remote-state-path-details-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-path-details-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-path-details-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing Remote Computer checked path detail evidence to fail" >&2
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
      "checked_path_count": 2,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-path-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-path-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-path-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate Remote Computer checked path detail evidence to fail" >&2
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
      "checked_path_count": 1,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "state_claim": "mandoforge-remote-computer-state"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-path-status-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-path-status-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-path-status-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing Remote Computer checked path status evidence to fail" >&2
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
      "checked_path_count": 1,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-remote-state-path-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-remote-state-path-audit-negative.out 2>/tmp/mandoforge-stage2-archive-remote-state-path-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing Remote Computer checked path audit evidence to fail" >&2
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
      "checked_path_count": 6,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "replacement_pods_healthy": true,
      "checked_pod_count": 1
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-sidecar-pod-details-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-sidecar-pod-details-negative.out 2>/tmp/mandoforge-stage2-archive-sidecar-pod-details-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing sidecar checked Pod detail evidence to fail" >&2
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
      "checked_pod_count": 2,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-sidecar-pod-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-sidecar-pod-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-sidecar-pod-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate sidecar checked Pod detail evidence to fail" >&2
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-sidecar-pod-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-sidecar-pod-audit-negative.out 2>/tmp/mandoforge-stage2-archive-sidecar-pod-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing sidecar checked Pod audit evidence to fail" >&2
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
  archive="$tmpdir/stage2-evidence-finance-delivery-receipts-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-delivery-receipts.out 2>/tmp/mandoforge-stage2-archive-finance-delivery-receipts.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing finance delivery receipt evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "latest_file_name": "mandoforge-usage-export.csv",
    "latest_bytes": 128,
    "delivery_count": 1,
    "delivery_receipts": [
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "netsuite-prod-1", "file_name": "mandoforge-usage-export.csv", "byte_count": 128, "status": "posted", "record_count": 1}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-delivery-receipt-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-delivery-receipt-audit.out 2>/tmp/mandoforge-stage2-archive-finance-delivery-receipt-audit.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing finance delivery receipt audit evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "latest_file_name": "mandoforge-usage-export.csv",
    "latest_bytes": 128,
    "delivery_count": 1,
    "delivery_receipts": [
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "quickbooks-prod-2", "file_name": "mandoforge-usage-export.csv", "byte_count": 128, "status": "posted", "record_count": 1, "posted_at": "1970-01-01T00:00:00Z", "audit_id": "finance-delivery-audit-1"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-delivery-receipt-system-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-delivery-receipt-system.out 2>/tmp/mandoforge-stage2-archive-finance-delivery-receipt-system.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched finance delivery receipt system evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "latest_file_name": "mandoforge-usage-export.csv",
    "latest_bytes": 128,
    "delivery_count": 1,
    "delivery_receipts": [
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "netsuite-prod-1", "file_name": "old-usage-export.csv", "byte_count": 64, "status": "posted", "record_count": 1, "posted_at": "1970-01-01T00:00:00Z", "audit_id": "finance-delivery-audit-1"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-delivery-receipt-export-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-delivery-receipt-export.out 2>/tmp/mandoforge-stage2-archive-finance-delivery-receipt-export.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched finance delivery receipt export evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "latest_file_name": "mandoforge-usage-export.csv",
    "latest_bytes": 128,
    "delivery_count": 2,
    "delivery_receipts": [
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "netsuite-prod-1", "file_name": "mandoforge-usage-export.csv", "byte_count": 128, "status": "posted", "record_count": 1, "posted_at": "1970-01-01T00:00:00Z", "audit_id": "finance-delivery-audit-1"},
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "netsuite-prod-1", "file_name": "mandoforge-usage-export.csv", "byte_count": 128, "status": "posted", "record_count": 1, "posted_at": "1970-01-01T00:00:01Z", "audit_id": "finance-delivery-audit-2"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-finance-delivery-receipt-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-finance-delivery-receipt-duplicate.out 2>/tmp/mandoforge-stage2-archive-finance-delivery-receipt-duplicate.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate finance delivery receipt evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/finance-export-delivery-observer.json" <<'JSON'
{
  "status": "ok",
  "export_state": {
    "delivery_mode": "netsuite",
    "system_id": "netsuite-prod-1",
    "latest_file_name": "mandoforge-usage-export.csv",
    "latest_bytes": 128,
    "delivery_count": 1,
    "delivery_receipts": [
      {"receipt_id": "netsuite-receipt-prod-1", "system_id": "netsuite-prod-1", "file_name": "mandoforge-usage-export.csv", "byte_count": 128, "status": "posted", "record_count": 1, "posted_at": "1970-01-01T00:00:00Z", "audit_id": "finance-delivery-audit-1"}
    ]
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
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"}
    ],
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
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"}
    ],
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
  archive="$tmpdir/stage2-evidence-vault-rotation-details-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-rotation-details-negative.out 2>/tmp/mandoforge-stage2-archive-vault-rotation-details-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing KMS rotation key detail evidence to fail" >&2
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
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true}
    ],
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-rotation-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-rotation-audit-negative.out 2>/tmp/mandoforge-stage2-archive-vault-rotation-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing KMS rotation audit evidence to fail" >&2
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
      "rotated_count": 2
    },
    "rotated_count": 2,
    "catalog_updated_count": 2,
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "record_id": "secret-record-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"},
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "record_id": "secret-record-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-2", "rotated_at": "1970-01-01T00:00:01Z"}
    ],
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-rotation-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-rotation-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-vault-rotation-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate KMS rotation detail evidence to fail" >&2
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
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"}
    ],
    "actions": ["rotation_audit_logged"]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-rotation-action-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-rotation-action-negative.out 2>/tmp/mandoforge-stage2-archive-vault-rotation-action-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing KMS rotation confirmation action evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/vault-kms-rotation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "external_execution": {
      "status": "validated",
      "production_backend": false,
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1",
      "rotation_id": "kms-rotation-1",
      "rotated_count": 1
    },
    "rotated_count": 1,
    "catalog_updated_count": 1,
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"}
    ],
    "actions": ["external_kms_rotation_confirmed"]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-production-backend-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-production-backend-negative.out 2>/tmp/mandoforge-stage2-archive-vault-production-backend-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected non-production KMS backend evidence to fail" >&2
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
    "rotation_details": [
      {"backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "rotation_id": "kms-rotation-1", "status": "rotated", "catalog_updated": true, "audit_id": "kms-rotation-audit-1", "rotated_at": "1970-01-01T00:00:00Z"}
    ],
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
        {"name": "restore-key-material", "status": "failed", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-recovery-step-status-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-recovery-step-status-negative.out 2>/tmp/mandoforge-stage2-archive-vault-recovery-step-status-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected invalid KMS recovery step status evidence to fail" >&2
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
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-recovery-step-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-recovery-step-audit-negative.out 2>/tmp/mandoforge-stage2-archive-vault-recovery-step-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing KMS recovery step audit evidence to fail" >&2
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
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-recovery-step-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-recovery-step-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-vault-recovery-step-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate KMS recovery step evidence to fail" >&2
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
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/other-key", "key_id": "other-key", "recovery_id": "kms-recovery-other", "audit_id": "kms-recovery-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-recovery-step-binding-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-recovery-step-binding-negative.out 2>/tmp/mandoforge-stage2-archive-vault-recovery-step-binding-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched KMS recovery step binding evidence to fail" >&2
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
      "recovery_target_kind": "pilot_kms_backend",
      "steps": [
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "verify-secret-consumers", "status": "passed", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-vault-recovery-target-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-recovery-target-negative.out 2>/tmp/mandoforge-stage2-archive-vault-recovery-target-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected non-production KMS recovery target evidence to fail" >&2
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
        {"name": "restore-key-material", "status": "validated", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "verify-secret-consumers", "status": "passed", "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1", "key_id": "key-1", "recovery_id": "kms-recovery-1", "audit_id": "kms-recovery-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
    },
    "vault_kms": {
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-2",
      "key_id": "key-2"
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
  archive="$tmpdir/stage2-evidence-vault-kms-target-mismatch-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-vault-kms-target-mismatch-negative.out 2>/tmp/mandoforge-stage2-archive-vault-kms-target-mismatch-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched KMS backend/key target evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-2",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
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
  archive="$tmpdir/stage2-evidence-policy-controller-target-mismatch-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-controller-target-mismatch-negative.out 2>/tmp/mandoforge-stage2-archive-policy-controller-target-mismatch-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched policy controller target evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-2",
      "deployment_id": "policy-deployment-prod-2"
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
  archive="$tmpdir/stage2-evidence-policy-store-deployment-target-mismatch-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-store-deployment-target-mismatch-negative.out 2>/tmp/mandoforge-stage2-archive-policy-store-deployment-target-mismatch-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched policy store/deployment target evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-2"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
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
  archive="$tmpdir/stage2-evidence-tenant-deployment-target-mismatch-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-deployment-target-mismatch-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-deployment-target-mismatch-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched tenant deployment target evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "prod-cluster-1",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
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
  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "validated",
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 1,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-rls-table-detail-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-rls-table-detail-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-rls-table-detail-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing forced-RLS table detail evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-duplicate-rls-table-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-duplicate-rls-table-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-duplicate-rls-table-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate forced-RLS table detail evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-rls-table-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-rls-table-audit-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-rls-table-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing forced-RLS table audit evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-2", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-2", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "blocked", "audit_id": "tenant-negative-b-a-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-deployment-detail-binding-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-deployment-detail-binding-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-deployment-detail-binding-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched tenant deployment detail evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-negative-test-details-missing.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-negative-test-details-missing.out 2>/tmp/mandoforge-stage2-archive-tenant-negative-test-details-missing.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing cross-tenant negative test detail evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-x", "target_tenant": "tenant-y", "status": "denied", "audit_id": "tenant-negative-x-y-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-y", "target_tenant": "tenant-x", "status": "blocked", "audit_id": "tenant-negative-y-x-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-negative-test-unsampled-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-negative-test-unsampled-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-negative-test-unsampled-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected unsampled cross-tenant negative test evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked-2", "checked_at": "1970-01-01T00:00:01Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-negative-test-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-negative-test-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-negative-test-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate cross-tenant negative test evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 1,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-negative-test-audit-missing.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-negative-test-audit-missing.out 2>/tmp/mandoforge-stage2-archive-tenant-negative-test-audit-missing.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing cross-tenant negative test audit evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": false,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-context-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-context-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-context-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing tenant context validation evidence to fail" >&2
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
      "tenant_count": 1,
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-count-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-count-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-count-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected single-tenant deployment evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-duplicate-sample-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-duplicate-sample-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-duplicate-sample-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate tenant sample evidence to fail" >&2
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
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-tenant-sample-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-tenant-sample-audit-negative.out 2>/tmp/mandoforge-stage2-archive-tenant-sample-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing tenant sample audit evidence to fail" >&2
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
      "tenant_samples": [
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-a", "status": "validated", "audit_id": "tenant-sample-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "tenant_id": "tenant-b", "status": "validated", "audit_id": "tenant-sample-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "rls_enforced": true,
      "rls_table_count": 2,
      "rls_forced_table_count": 2,
      "rls_table_details": [
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "sessions", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "schema": "public", "table": "session_events", "rls_enabled": true, "rls_forced": true, "audit_id": "rls-table-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ],
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true,
      "cross_tenant_negative_test_count": 2,
      "cross_tenant_negative_test_results": [
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "denied", "audit_id": "tenant-negative-a-b-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-b", "target_tenant": "tenant-a", "status": "denied", "audit_id": "tenant-negative-b-a-denied", "checked_at": "1970-01-01T00:00:00Z"},
        {"deployment_id": "tenant-routing-prod-1", "source_tenant": "tenant-a", "target_tenant": "tenant-b", "status": "blocked", "audit_id": "tenant-negative-a-b-blocked", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "staged-runtime-clear", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-2", "checked_at": "1970-01-01T00:00:00Z"}
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
      "steps": [
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "staged-runtime-clear", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-rollback-id-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-rollback-id-negative.out 2>/tmp/mandoforge-stage2-archive-policy-rollback-id-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy rollback evidence id to fail" >&2
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "staged-runtime-clear", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-rollback-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-rollback-audit-negative.out 2>/tmp/mandoforge-stage2-archive-policy-rollback-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy rollback audit evidence to fail" >&2
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "status": "failed"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-step-status-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-step-status-negative.out 2>/tmp/mandoforge-stage2-archive-policy-step-status-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected invalid policy rollout step status evidence to fail" >&2
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "status": "passed"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-step-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-step-audit-negative.out 2>/tmp/mandoforge-stage2-archive-policy-step-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy rollout step audit evidence to fail" >&2
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "controller_id": "policy-controller-other", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-step-binding-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-step-binding-negative.out 2>/tmp/mandoforge-stage2-archive-policy-step-binding-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched policy rollout step binding evidence to fail" >&2
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-2", "checked_at": "1970-01-01T00:00:00Z"}
      ]
    }
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-step-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-step-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-policy-step-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate policy rollout step evidence to fail" >&2
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
      "rollback_plan_id": "policy-rollback-plan-prod-1",
      "rollback_checked_at": "1970-01-01T00:00:00Z",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1",
      "steps": [
        {"name": "due-run-supervision", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"name": "staged-runtime-clear", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "passed", "audit_id": "policy-audit-prod-2", "checked_at": "1970-01-01T00:00:00Z"}
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
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
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
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 1,
    "skipped_count": 1,
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-due-run-scan-detail-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-due-run-scan-detail-negative.out 2>/tmp/mandoforge-stage2-archive-policy-due-run-scan-detail-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy due-run scan detail evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 2,
    "skipped_count": 0,
    "scanned_revisions": [
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned", "audit_id": "policy-due-run-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned", "audit_id": "policy-due-run-audit-2", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-due-run-scan-duplicate-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-due-run-scan-duplicate-negative.out 2>/tmp/mandoforge-stage2-archive-policy-due-run-scan-duplicate-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected duplicate policy due-run scan detail evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 1,
    "skipped_count": 1,
    "scanned_revisions": [
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned"}
    ],
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-due-run-scan-audit-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-due-run-scan-audit-negative.out 2>/tmp/mandoforge-stage2-archive-policy-due-run-scan-audit-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy due-run scan audit evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 1,
    "skipped_count": 1,
    "scanned_revisions": [
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-other", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned", "audit_id": "policy-due-run-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "checked_at": "1970-01-01T00:00:00Z"
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-due-run-scan-binding-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-due-run-scan-binding-negative.out 2>/tmp/mandoforge-stage2-archive-policy-due-run-scan-binding-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected mismatched policy due-run scan binding evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 1,
    "skipped_count": 1,
    "scanned_revisions": [
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned", "audit_id": "policy-due-run-audit-1", "scanned_at": "1970-01-01T00:00:00Z"}
    ],
    "checked_at": ""
  }
}
JSON
  archive="$tmpdir/stage2-evidence-policy-due-run-checked-at-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-policy-due-run-checked-at-negative.out 2>/tmp/mandoforge-stage2-archive-policy-due-run-checked-at-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected missing policy due-run checked_at evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/policy-rollout-due-run-evidence.json" <<'JSON'
{
  "status": "captured",
  "response": {
    "status": "noop",
    "controller_id": "policy-controller-prod-1",
    "policy_store_id": "policy-store-prod-1",
    "deployment_id": "policy-deployment-prod-1",
    "scanned_count": 1,
    "skipped_count": 1,
    "scanned_revisions": [
      {"policy_id": "policy-prod-1", "revision_id": "policy-revision-prod-1", "controller_id": "policy-controller-prod-1", "policy_store_id": "policy-store-prod-1", "deployment_id": "policy-deployment-prod-1", "status": "scanned", "audit_id": "policy-due-run-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-canary",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-canary", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-worker-pool-mismatch.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-worker-pool-negative.out 2>/tmp/mandoforge-stage2-archive-summary-worker-pool-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected combined summary worker_pool mismatch to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "different-prod-state-claim",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-state-claim-mismatch.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-state-claim-negative.out 2>/tmp/mandoforge-stage2-archive-summary-state-claim-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected combined summary state claim mismatch to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "cephfs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-state-backend-mismatch.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-state-backend-negative.out 2>/tmp/mandoforge-stage2-archive-summary-state-backend-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected combined summary state backend mismatch to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "different-prod-cluster",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "different-prod-cluster",
    "sidecar_cluster_id": "different-prod-cluster",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
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
  "same_cluster_target": false,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-shared-cluster-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-shared-cluster-negative.out 2>/tmp/mandoforge-stage2-archive-summary-shared-cluster-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary without shared cluster proof to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 1,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "different-prod-state-claim", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-path-claim-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-path-claim-negative.out 2>/tmp/mandoforge-stage2-archive-summary-path-claim-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary checked path claim mismatch to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 2,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-duplicate-path-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-duplicate-path-negative.out 2>/tmp/mandoforge-stage2-archive-summary-duplicate-path-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary duplicate checked path detail evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 1,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 2,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-duplicate-sidecar-pod-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-duplicate-sidecar-pod-negative.out 2>/tmp/mandoforge-stage2-archive-summary-duplicate-sidecar-pod-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary duplicate sidecar checked Pod detail evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "different-prod-cluster", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 1,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-detail-cluster-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-detail-cluster-negative.out 2>/tmp/mandoforge-stage2-archive-summary-detail-cluster-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary detail cluster mismatch to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-worker-load-check-details-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-worker-load-check-details-negative.out 2>/tmp/mandoforge-stage2-archive-summary-worker-load-check-details-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary without worker load-check detail evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 2,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-2", "checked_at": "1970-01-01T00:00:01Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 1,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON
  archive="$tmpdir/stage2-evidence-summary-duplicate-worker-load-check-negative.tar.gz"
  tar czf "$archive" -C "$tmpdir/evidence" .
  sha="$(sha256_value "$archive")"
  printf '%s  %s\n' "$sha" "$archive" >"${archive}.sha256"
  {
    echo "created_at=1970-01-01T00:00:00Z"
    echo "archive_path=$archive"
    echo "archive_sha256=$sha"
  } >"${archive}.manifest.txt"
  set +e
  "$0" "$archive" >/tmp/mandoforge-stage2-archive-summary-duplicate-worker-load-check-negative.out 2>/tmp/mandoforge-stage2-archive-summary-duplicate-worker-load-check-negative.err
  negative_status="$?"
  set -e
  if [[ "$negative_status" == "0" ]]; then
    echo "Stage 2 archive verifier self-test expected summary duplicate worker load check detail evidence to fail" >&2
    exit 1
  fi

  cat >"$tmpdir/evidence/worker-remote-computer/summary.json" <<'JSON'
{
  "status": "ready",
  "production_blocked": false,
  "production_blocked_count": 0,
  "same_cluster_target": true,
  "worker": {
    "cluster_id": "prod-cluster-1",
    "worker_pool": "managed-agents-prod",
    "load_check_detail_count": 1,
    "load_checks": [
      {"cluster_id": "prod-cluster-1", "name": "queue-isolated", "worker_pool": "managed-agents-prod", "status": "validated", "audit_id": "worker-load-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  },
  "remote_computer": {
    "state_sync_cluster_id": "prod-cluster-1",
    "sidecar_cluster_id": "prod-cluster-1",
    "distributed_state_backend": "juicefs",
    "state_claim": "mandoforge-remote-computer-state",
    "checked_path_count": 6,
    "checked_paths": [
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
      {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ],
    "replacement_pods_healthy": true,
    "checked_pod_count": 1,
    "checked_pods": [
      {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
    ]
  }
}
JSON

  cat >"$tmpdir/evidence/production-evidence-run.json" <<'JSON'
{
  "generated_at": "1970-01-01T00:00:00Z",
  "source": "stage2-production-evidence-gate",
  "expected_targets": {
    "worker_remote_computer": {
      "cluster_id": "whiskey-pilot-cluster",
      "worker_pool": "managed-agents-prod",
      "state_claim": "mandoforge-remote-computer-state"
    },
    "tenant_routing": {
      "deployment_id": "tenant-routing-prod-1"
    },
    "policy_rollout": {
      "controller_id": "policy-controller-prod-1",
      "policy_store_id": "policy-store-prod-1",
      "deployment_id": "policy-deployment-prod-1"
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
      "checked_path_count": 6,
      "checked_paths": [
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/session-events", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/runtime-turns", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/artifacts", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/audit-log", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/leases", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"},
        {"cluster_id": "prod-cluster-1", "path": "/agent-state/checkpoints", "status": "validated", "state_claim": "mandoforge-remote-computer-state", "audit_id": "state-path-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
      "checked_pod_count": 1,
      "checked_pods": [
        {"cluster_id": "prod-cluster-1", "pod": "remote-computer-sidecar-prod-1", "status": "running", "audit_id": "sidecar-pod-audit-1", "checked_at": "1970-01-01T00:00:00Z"}
      ]
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
  export ALLOW_LEGACY_STAGE2_ARCHIVE_MANIFEST=1
  export MANDOFORGE_STAGE2_ARCHIVE_SELF_TEST=1
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
