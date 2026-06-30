#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/observability-ops-production-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/observability-ops-production}"
OPS_EVIDENCE_FILE="${OBSERVABILITY_OPS_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/summary.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "observability ops production gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "observability ops production gate failed: $*" >&2
  exit 1
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable script: $1"
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost|sandbox-only)([./:_-]|$) ]] || return 1
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
  require_executable scripts/observability-collector-evidence-gate.sh
  require_executable scripts/finance-evidence-gate.sh
  require_executable scripts/observability-ops-production-gate.sh

  grep -q "observability-ops-production-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the observability ops production gate"
  grep -q "observability-ops-production-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the observability ops production gate"
  grep -q "observability-ops-production-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the observability ops production gate"
  grep -q "observability-ops-production-gate.sh" deploy/stage2-evidence/observability-ops-production-job.example.yaml \
    || fail "observability ops production Job must run the dedicated gate"
  grep -q "observability-ops-production-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the observability ops production Job"
  grep -q "observability-collector-remediation-evidence.json" scripts/observability-collector-evidence-gate.sh \
    || fail "collector evidence gate must capture remediation evidence"
  grep -q "/api/usage/finance-operations/summary" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must expose finance operations readiness for observability"
}

ops_issue() {
  local artifact="$1"
  local status target_id target_kind audit_id checked_at support_owner archive_uri immutable

  [[ -s "$artifact" ]] || {
    printf 'missing observability ops production evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"
  support_owner="$(jq -r '.support_owner // .operations_owner // .oncall_owner // ""' "$artifact")"
  archive_uri="$(jq -r '.evidence_archive.uri // .archive.uri // ""' "$artifact")"
  immutable="$(jq -r '.evidence_archive.immutable // .archive.immutable // false' "$artifact")"

  if ! ready_value "$status"; then
    printf 'observability ops production status is not ready: %s' "$status"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'observability ops target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  case "$target_kind" in
    production_observability|production_runtime_cluster|managed_agent_cluster|kubernetes_cluster|customer_grade_deployment) ;;
    *)
      printf 'observability ops target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
    printf 'observability ops evidence lacks audit, timestamp, or support owner'
    return 0
  fi
  if [[ "$immutable" != "true" || -z "$archive_uri" ]]; then
    printf 'observability ops evidence lacks immutable archive metadata'
    return 0
  fi

  jq -e '
    def ready: . == "ready" or . == "validated" or . == "completed" or . == "passed";
    (.correlation.status // "unknown" | ready)
    and ((.correlation.fields // []) | index("tenant_id") != null)
    and ((.correlation.fields // []) | index("session_id") != null)
    and ((.correlation.fields // []) | index("workflow_run_id") != null)
    and ((.correlation.fields // []) | index("tool_call_id") != null)
    and ((.correlation.fields // []) | index("worker_id") != null)
    and ((.correlation.fields // []) | index("connector_id") != null)
    and ((.correlation.fields // []) | index("provider_id") != null)
    and (.alerts.status // "unknown" | ready)
    and ((.alerts.coverage // []) | index("failed_jobs") != null)
    and ((.alerts.coverage // []) | index("stale_leases") != null)
    and ((.alerts.coverage // []) | index("delivery_failures") != null)
    and ((.alerts.coverage // []) | index("connector_degradation") != null)
    and ((.alerts.coverage // []) | index("provider_degradation") != null)
    and ((.alerts.coverage // []) | index("budget_breach") != null)
    and ((.alerts.coverage // []) | index("queue_backlog") != null)
    and (.alerts.delivery_tested == true)
    and (.versions.status // "unknown" | ready)
    and ((.versions.visible // []) | index("deployment") != null)
    and ((.versions.visible // []) | index("migration") != null)
    and ((.versions.visible // []) | index("workflow_pack") != null)
    and ((.versions.visible // []) | index("ontology") != null)
    and ((.versions.visible // []) | index("connector") != null)
    and (.incident_timeline.status // "unknown" | ready)
    and (.incident_timeline.audit_captured == true)
    and (.manual_repair.status // "unknown" | ready)
    and (.manual_repair.actions_audited == true)
    and (.manual_repair.replay_tested == true)
    and (.slos.status // "unknown" | ready)
    and ((.slos.coverage // []) | index("runtime") != null)
    and ((.slos.coverage // []) | index("connector") != null)
    and ((.slos.coverage // []) | index("worker") != null)
    and ((.slos.coverage // []) | index("approval") != null)
    and ((.slos.coverage // []) | index("remote_computer") != null)
    and (.runbooks.status // "unknown" | ready)
    and (.runbooks.rehearsed == true)
    and (.runbooks.owner_acknowledged == true)
  ' "$artifact" >/dev/null || {
    printf 'observability ops production evidence is incomplete in %s' "$artifact"
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
    --arg source "observability-ops-production-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg ops_evidence_file "$OPS_EVIDENCE_FILE" \
    --arg issue "$issue" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      ops_evidence_file: $ops_evidence_file,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "observability_ops_production_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "ops_evidence_file=$OPS_EVIDENCE_FILE"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "observability ops production static gate ok"
  exit 0
fi

blocked_count=0
issue=""
if issue_text="$(ops_issue "$OPS_EVIDENCE_FILE")"; then
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

echo "observability ops production gate ok"
