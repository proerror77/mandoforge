#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/workflowpack-enterprise-lifecycle-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
WORKFLOWPACK_LIFECYCLE_EVIDENCE_FILE="${WORKFLOWPACK_ENTERPRISE_LIFECYCLE_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/workflowpack-enterprise-lifecycle/summary.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "workflowpack enterprise lifecycle gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "workflowpack enterprise lifecycle gate failed: $*" >&2
  exit 1
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable script: $1"
}

ready_value() {
  case "$1" in
    ready|validated|completed|passed)
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
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

static_contract_check() {
  require_executable scripts/verify-workflow-pack-manifest.sh
  require_executable scripts/workflow-pack-evidence-gate.sh
  require_executable scripts/managed-workflow-runtime-evidence-gate.sh
  require_executable scripts/workflowpack-enterprise-lifecycle-gate.sh

  grep -q "workflowpack-enterprise-lifecycle-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the WorkflowPack enterprise lifecycle gate"
  grep -q "workflowpack-enterprise-lifecycle-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the WorkflowPack enterprise lifecycle gate"
  grep -q "workflowpack-enterprise-lifecycle-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the WorkflowPack enterprise lifecycle gate"
  grep -q "workflowpack-enterprise-lifecycle-gate.sh" deploy/stage2-evidence/workflowpack-enterprise-lifecycle-job.example.yaml \
    || fail "WorkflowPack enterprise lifecycle Job must run the dedicated gate"
  grep -q "workflowpack-enterprise-lifecycle-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the WorkflowPack enterprise lifecycle Job"
}

lifecycle_issue() {
  local artifact="$1"
  local status evidence_class target_id target_kind target_environment pack_id pack_version audit_id checked_at support_owner
  local archive_uri immutable archive_digest retention_policy

  [[ -s "$artifact" ]] || {
    printf 'missing WorkflowPack enterprise lifecycle evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  evidence_class="$(jq -r '.evidence_class // .required_evidence_class // ""' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  target_environment="$(jq -r '.target.environment // ""' "$artifact")"
  pack_id="$(jq -r '.pack.id // .workflow_pack.id // ""' "$artifact")"
  pack_version="$(jq -r '.pack.version // .workflow_pack.version // ""' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"
  support_owner="$(jq -r '.support_owner // .pack_owner // .oncall_owner // ""' "$artifact")"
  archive_uri="$(jq -r '.evidence_archive.uri // .archive.uri // ""' "$artifact")"
  immutable="$(jq -r '.evidence_archive.immutable // .archive.immutable // false' "$artifact")"
  archive_digest="$(jq -r '.evidence_archive.digest // .archive.digest // ""' "$artifact")"
  retention_policy="$(jq -r '.evidence_archive.retention_policy // .archive.retention_policy // ""' "$artifact")"

  if ! ready_value "$status"; then
    printf 'WorkflowPack enterprise lifecycle status is not ready: %s' "$status"
    return 0
  fi
  if [[ "$evidence_class" != "customer_grade" ]]; then
    printf 'WorkflowPack enterprise lifecycle evidence class is not customer_grade: %s' "${evidence_class:-<empty>}"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'WorkflowPack enterprise lifecycle target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  if [[ "$target_environment" != "production" || -z "$pack_id" || -z "$pack_version" ]]; then
    printf 'WorkflowPack enterprise lifecycle evidence lacks production environment, pack id, or pack version'
    return 0
  fi
  case "$target_kind" in
    workflowpack_lifecycle|production_workflowpack|customer_grade_deployment|kubernetes_cluster|managed_agent_cluster) ;;
    *)
      printf 'WorkflowPack enterprise lifecycle target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
    printf 'WorkflowPack enterprise lifecycle evidence lacks audit, timestamp, or support owner'
    return 0
  fi
  if [[ "$immutable" != "true" || -z "$archive_uri" || -z "$archive_digest" || -z "$retention_policy" ]]; then
    printf 'WorkflowPack enterprise lifecycle evidence lacks immutable archive URI, digest, or retention metadata'
    return 0
  fi

  jq -e '
    .checks.install_audited == true
    and .checks.stage_audited == true
    and .checks.release_promoted == true
    and .checks.rollback_verified == true
    and .checks.archive_verified == true
    and .checks.onboarding_profiles_complete == true
    and .checks.connector_quality_passed == true
    and .checks.eval_regression_passed == true
    and .checks.canary_promoted == true
    and .checks.compatibility_matrix_passed == true
    and .checks.tenant_overrides_policy_enforced == true
    and .checks.managed_workflow_recovery_passed == true
    and ((.compatibility_matrix.versions // []) | length > 0)
    and ((.tenant_overrides.validated_tenants // []) | length > 0)
    and ((.managed_workflow_runtime.recovery_checks // []) | length > 0)
  ' "$artifact" >/dev/null || {
    printf 'WorkflowPack enterprise lifecycle summary is incomplete'
    return 0
  }

  return 1
}

write_summary() {
  local status="$1"
  local blocked_count="$2"
  local issue="$3"
  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg source "workflowpack-enterprise-lifecycle-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg evidence_file "$WORKFLOWPACK_LIFECYCLE_EVIDENCE_FILE" \
    --arg issue "$issue" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      workflowpack_enterprise_lifecycle_evidence_file: $evidence_file,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "workflowpack_enterprise_lifecycle_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "workflowpack_enterprise_lifecycle_evidence_file=$WORKFLOWPACK_LIFECYCLE_EVIDENCE_FILE"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "workflowpack enterprise lifecycle static gate ok"
  exit 0
fi

blocked_count=0
issue=""
if issue_text="$(lifecycle_issue "$WORKFLOWPACK_LIFECYCLE_EVIDENCE_FILE")"; then
  echo "$issue_text" >&2
  issue="$issue_text"
  blocked_count=1
fi

if [[ "$blocked_count" == "0" ]]; then
  write_summary "ready" 0 ""
else
  write_summary "blocked" "$blocked_count" "$issue"
fi

cat "$EVIDENCE_DIR/summary.txt"

if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
  exit 1
fi

echo "workflowpack enterprise lifecycle gate ok"
