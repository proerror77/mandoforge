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
      local target_kind
      local node_count
      local cluster_id
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.controller_execution.node_count // 0' "$path")"
      cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$path")"
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
      ;;
    remote-computer-state-sync-evidence.json)
      local target_kind
      local node_count
      local state_backend
      local cluster_id
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.controller_execution.node_count // 0' "$path")"
      state_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // "unknown"' "$path")"
      cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$path")"
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
      ;;
    remote-computer-sidecar-recovery-evidence.json)
      local status
      local target_kind
      local node_count
      local replacement_scope
      local cluster_id
      status="$(jq -r '.response.validation_result.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.validation_result.target_kind // "unknown"' "$path")"
      node_count="$(jq -r '.response.validation_result.node_count // 0' "$path")"
      replacement_scope="$(jq -r '.response.validation_result.replacement_scope // "unknown"' "$path")"
      cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$path")"
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
      ;;
    tenant-routing-validation-evidence.json)
      local target_kind
      local tenant_count
      local rls_enforced
      local tenant_context_validated
      local cross_tenant_negative_tests
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      tenant_count="$(jq -r '.response.controller_execution.tenant_count // 0' "$path")"
      rls_enforced="$(jq -r '.response.controller_execution.rls_enforced // false' "$path")"
      tenant_context_validated="$(jq -r '.response.controller_execution.tenant_context_validated // false' "$path")"
      cross_tenant_negative_tests="$(jq -r '.response.controller_execution.cross_tenant_negative_tests // false' "$path")"
      if ! is_multi_tenant_target_kind "$target_kind"; then
        printf '%s target_kind=%s is not broader multi-tenant' "$relative_path" "$target_kind"
        return 0
      fi
      if [[ ! "$tenant_count" =~ ^[0-9]+$ || "$tenant_count" -lt 2 ]]; then
        printf '%s tenant_count=%s is not multi-tenant' "$relative_path" "$tenant_count"
        return 0
      fi
      if [[ "$rls_enforced" != "true" || "$tenant_context_validated" != "true" || "$cross_tenant_negative_tests" != "true" ]]; then
        printf '%s tenant/RLS negative-test evidence incomplete' "$relative_path"
        return 0
      fi
      ;;
    policy-rollout-orchestration-validation-evidence.json)
      local status
      local controller_status
      local target_kind
      local environment
      local controller_id
      local rollout_scope
      local production_policy_store
      local rollback_supported
      status="$(jq -r '.response.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$path")"
      controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$path")"
      rollout_scope="$(jq -r '.response.controller_execution.rollout_scope // "unknown"' "$path")"
      production_policy_store="$(jq -r '.response.controller_execution.production_policy_store // false' "$path")"
      rollback_supported="$(jq -r '.response.controller_execution.rollback_supported // false' "$path")"
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
      if ! is_production_rollout_scope "$rollout_scope"; then
        printf '%s rollout_scope=%s is not production-grade' "$relative_path" "$rollout_scope"
        return 0
      fi
      if [[ "$production_policy_store" != "true" || "$rollback_supported" != "true" ]]; then
        printf '%s policy store or rollback evidence incomplete' "$relative_path"
        return 0
      fi
      ;;
    vault-kms-rotation-evidence.json)
      local status
      local execution_status
      local production_backend
      local backend_kind
      local environment
      local backend_id
      local key_id
      status="$(jq -r '.response.status // "unknown"' "$path")"
      execution_status="$(jq -r '.response.external_execution.status // "unknown"' "$path")"
      production_backend="$(jq -r '.response.external_execution.production_backend // false' "$path")"
      backend_kind="$(jq -r '.response.external_execution.backend_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.external_execution.environment // "unknown"' "$path")"
      backend_id="$(jq -r '.response.external_execution.backend_id // ""' "$path")"
      key_id="$(jq -r '.response.external_execution.key_id // ""' "$path")"
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
      ;;
    vault-kms-recovery-evidence.json)
      local status
      local controller_status
      local backend_kind
      local environment
      local backend_id
      local key_id
      status="$(jq -r '.response.status // "unknown"' "$path")"
      controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$path")"
      backend_kind="$(jq -r '.response.controller_execution.backend_kind // "unknown"' "$path")"
      environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$path")"
      backend_id="$(jq -r '.response.controller_execution.backend_id // ""' "$path")"
      key_id="$(jq -r '.response.controller_execution.key_id // ""' "$path")"
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

verify_run_manifest() {
  local root="$1"
  local manifest="$root/production-evidence-run.json"
  local issue_count=0
  local issue
  local expected_cluster_id
  local worker_cluster_id
  local state_cluster_id
  local sidecar_cluster_id
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

  if [[ ! -s "$manifest" ]]; then
    echo "Stage 2 evidence archive semantic issue: production-evidence-run.json missing" >&2
    return 1
  fi

  expected_cluster_id="$(jq -r '.expected_targets.worker_remote_computer.cluster_id // ""' "$manifest")"
  worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/worker-load-validation-evidence.json" 2>/dev/null || echo "")"
  state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$root/remote-computer-state-sync-evidence.json" 2>/dev/null || echo "")"
  sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$root/remote-computer-sidecar-recovery-evidence.json" 2>/dev/null || echo "")"
  for issue in \
    "$(value_mismatch_issue "worker cluster id" "$expected_cluster_id" "$worker_cluster_id")" \
    "$(value_mismatch_issue "remote state-sync cluster id" "$expected_cluster_id" "$state_cluster_id")" \
    "$(value_mismatch_issue "remote sidecar cluster id" "$expected_cluster_id" "$sidecar_cluster_id")"; do
    if [[ -n "$issue" ]]; then
      issue_count=$((issue_count + 1))
      echo "Stage 2 evidence archive semantic issue: $issue" >&2
    fi
  done

  expected_tenant_deployment_id="$(jq -r '.expected_targets.tenant_routing.deployment_id // ""' "$manifest")"
  tenant_deployment_id="$(jq -r '.response.controller_execution.deployment_id // ""' "$root/tenant-routing-validation-evidence.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "tenant routing deployment id" "$expected_tenant_deployment_id" "$tenant_deployment_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi

  expected_policy_controller_id="$(jq -r '.expected_targets.policy_rollout.controller_id // ""' "$manifest")"
  policy_controller_id="$(jq -r '.response.controller_execution.controller_id // ""' "$root/policy-rollout-orchestration-validation-evidence.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "policy rollout controller id" "$expected_policy_controller_id" "$policy_controller_id")"
  if [[ -n "$issue" ]]; then
    issue_count=$((issue_count + 1))
    echo "Stage 2 evidence archive semantic issue: $issue" >&2
  fi

  expected_kms_backend_id="$(jq -r '.expected_targets.vault_kms.backend_id // ""' "$manifest")"
  expected_kms_key_id="$(jq -r '.expected_targets.vault_kms.key_id // ""' "$manifest")"
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
  finance_system_id="$(jq -r '.export_state.system_id // .export_state.erp_system_id // .export_state.accounting_system_id // .export_state.target_id // ""' "$root/finance-export-delivery-observer.json" 2>/dev/null || echo "")"
  issue="$(value_mismatch_issue "finance ERP system id" "$expected_finance_system_id" "$finance_system_id")"
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
    tenant-routing-validation-evidence.json
    policy-rollout-orchestration-validation-evidence.json
    vault-kms-rotation-evidence.json
    vault-kms-recovery-evidence.json
    finance-export-delivery-observer.json
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
  "response": {
    "controller_execution": {
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1"
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
    }
  }
}
JSON
  cat >"$tmpdir/evidence/remote-computer-state-sync-evidence.json" <<'JSON'
{
  "response": {
    "controller_execution": {
      "target_kind": "k8s_cluster",
      "node_count": 3,
      "cluster_id": "prod-cluster-1",
      "distributed_state_backend": "juicefs"
    }
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
      "cluster_id": "prod-cluster-1",
      "replacement_scope": "cluster"
    }
  }
}
JSON
  cat >"$tmpdir/evidence/tenant-routing-validation-evidence.json" <<'JSON'
{
  "response": {
    "controller_execution": {
      "target_kind": "production_multi_tenant",
      "deployment_id": "tenant-routing-prod-1",
      "tenant_count": 2,
      "rls_enforced": true,
      "tenant_context_validated": true,
      "cross_tenant_negative_tests": true
    }
  }
}
JSON
  cat >"$tmpdir/evidence/policy-rollout-orchestration-validation-evidence.json" <<'JSON'
{
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "target_kind": "production_policy_controller",
      "environment": "production",
      "controller_id": "policy-controller-prod-1",
      "rollout_scope": "global",
      "production_policy_store": true,
      "rollback_supported": true
    }
  }
}
JSON
  cat >"$tmpdir/evidence/vault-kms-rotation-evidence.json" <<'JSON'
{
  "response": {
    "status": "validated",
    "external_execution": {
      "status": "validated",
      "production_backend": true,
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1"
    }
  }
}
JSON
  cat >"$tmpdir/evidence/vault-kms-recovery-evidence.json" <<'JSON'
{
  "response": {
    "status": "validated",
    "controller_execution": {
      "status": "validated",
      "backend_kind": "aws_kms",
      "environment": "production",
      "backend_id": "arn:aws:kms:us-east-1:111122223333:key/key-1",
      "key_id": "key-1"
    }
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
  local negative_status="$?"
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
    "delivery_count": 1
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
