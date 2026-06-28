#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/enterprise-security-production-controls-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/enterprise-security-production-controls}"
CONTROLS_EVIDENCE_FILE="${ENTERPRISE_SECURITY_CONTROLS_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/summary.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "enterprise security production controls gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "enterprise security production controls gate failed: $*" >&2
  exit 1
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable script: $1"
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
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

static_contract_check() {
  require_executable scripts/enterprise-security-admin-readiness-gate.sh
  require_executable scripts/tenant-isolation-evidence-gate.sh
  require_executable scripts/vault-evidence-gate.sh
  require_executable scripts/approval-notification-evidence-gate.sh
  require_executable scripts/enterprise-security-production-controls-gate.sh

  grep -q "enterprise-security-production-controls-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the production controls gate"
  grep -q "enterprise-security-production-controls-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the production controls gate"
  grep -q "enterprise-security-production-controls-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the production controls gate"
  grep -q "enterprise-security-production-controls-gate.sh" deploy/stage2-evidence/enterprise-security-production-controls-job.example.yaml \
    || fail "enterprise security production controls Job must run the dedicated gate"
  grep -q "enterprise-security-production-controls-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the enterprise security production controls Job"

  grep -q "identity-provisioning" crates/mandoforge-api/src/enterprise_security_readiness.rs \
    || fail "enterprise security readiness must include identity provisioning"
  grep -q "audit-export-siem" crates/mandoforge-api/src/enterprise_security_readiness.rs \
    || fail "enterprise security readiness must include SIEM audit export"
  grep -q "data-governance" crates/mandoforge-api/src/enterprise_security_readiness.rs \
    || fail "enterprise security readiness must include data governance"
  grep -q "approval-break-glass" crates/mandoforge-api/src/enterprise_security_readiness.rs \
    || fail "enterprise security readiness must include break-glass controls"
}

controls_issue() {
  local artifact="$1"
  local status target_id target_kind audit_id checked_at support_owner control_count

  [[ -s "$artifact" ]] || {
    printf 'missing enterprise security controls evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"
  support_owner="$(jq -r '.support_owner // .security_owner // .oncall_owner // ""' "$artifact")"
  control_count="$(jq -r '[.controls[]?] | length' "$artifact")"

  if ! ready_value "$status"; then
    printf 'enterprise security controls status is not ready: %s' "$status"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'enterprise security target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  case "$target_kind" in
    production_security_controls|production_runtime_cluster|managed_agent_cluster|kubernetes_cluster|customer_grade_deployment) ;;
    *)
      printf 'enterprise security target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
    printf 'enterprise security controls evidence lacks audit, timestamp, or support owner'
    return 0
  fi
  if [[ "$control_count" -lt 7 ]]; then
    printf 'enterprise security controls evidence must include all required control families'
    return 0
  fi

  jq -e '
    def ready: . == "ready" or . == "validated" or . == "completed" or . == "passed";
    def control($id): first(.controls[]? | select(.id == $id));
    (control("identity-provisioning").status // "unknown" | ready)
    and ((control("identity-provisioning").sso_protocol // "") == "oidc" or (control("identity-provisioning").sso_protocol // "") == "saml")
    and (control("identity-provisioning").scim_enabled == true)
    and ((control("identity-provisioning").directory_id // "") | length > 0)
    and (control("tenant-rls-abac").status // "unknown" | ready)
    and (control("tenant-rls-abac").rls_forced == true)
    and (control("tenant-rls-abac").abac_tested == true)
    and (control("vault-kms-rotation-recovery").status // "unknown" | ready)
    and (control("vault-kms-rotation-recovery").production_kms_backend == true)
    and (control("vault-kms-rotation-recovery").rotation_tested == true)
    and (control("vault-kms-rotation-recovery").recovery_tested == true)
    and (control("approval-break-glass").status // "unknown" | ready)
    and (control("approval-break-glass").break_glass_tested == true)
    and (control("approval-break-glass").audit_captured == true)
    and (control("audit-export-siem").status // "unknown" | ready)
    and (control("audit-export-siem").delivery_tested == true)
    and ((control("audit-export-siem").correlation_fields // []) | index("tenant_id") != null)
    and ((control("audit-export-siem").correlation_fields // []) | index("session_id") != null)
    and ((control("audit-export-siem").correlation_fields // []) | index("tool_call_id") != null)
    and (control("data-governance").status // "unknown" | ready)
    and (control("data-governance").retention_tested == true)
    and (control("data-governance").legal_hold_tested == true)
    and (control("data-governance").export_tested == true)
    and (control("data-governance").deletion_tested == true)
    and (control("data-governance").pii_redaction_tested == true)
    and (control("data-governance").dlp_tested == true)
    and (control("security-incident-operations").status // "unknown" | ready)
    and (control("security-incident-operations").runbook_rehearsed == true)
    and (control("security-incident-operations").evidence_archive_immutable == true)
  ' "$artifact" >/dev/null || {
    printf 'enterprise security controls evidence is incomplete in %s' "$artifact"
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
    --arg status "$status" \
    --arg controls_evidence_file "$CONTROLS_EVIDENCE_FILE" \
    --arg issue "$issue" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      status: $status,
      controls_evidence_file: $controls_evidence_file,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "enterprise_security_production_controls_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "controls_evidence_file=$CONTROLS_EVIDENCE_FILE"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "enterprise security production controls static gate ok"
  exit 0
fi

blocked_count=0
issue=""
if issue_text="$(controls_issue "$CONTROLS_EVIDENCE_FILE")"; then
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

echo "enterprise security production controls gate ok"
