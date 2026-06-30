#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/ontology-engine-production-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
ONTOLOGY_ENGINE_EVIDENCE_FILE="${ONTOLOGY_ENGINE_PRODUCTION_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/ontology-engine-production/summary.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ontology engine production gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "ontology engine production gate failed: $*" >&2
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
  require_executable scripts/ontology-engine-readiness-gate.sh
  require_executable scripts/ontology-release-loop-gate.sh
  require_executable scripts/ontology-release-workflow-trigger-gate.sh
  require_executable scripts/ontology-engine-production-gate.sh

  grep -q "ontology-engine-production-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the ontology engine production gate"
  grep -q "ontology-engine-production-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the ontology engine production gate"
  grep -q "ontology-engine-production-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the ontology engine production gate"
  grep -q "ontology-engine-production-gate.sh" deploy/stage2-evidence/ontology-engine-production-job.example.yaml \
    || fail "ontology engine production Job must run the dedicated gate"
  grep -q "ontology-engine-production-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the ontology engine production Job"
}

ontology_issue() {
  local artifact="$1"
  local status evidence_class target_id target_kind target_environment registry_version audit_id checked_at support_owner
  local archive_uri immutable archive_digest retention_policy

  [[ -s "$artifact" ]] || {
    printf 'missing ontology engine production evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  evidence_class="$(jq -r '.evidence_class // .required_evidence_class // ""' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  target_environment="$(jq -r '.target.environment // ""' "$artifact")"
  registry_version="$(jq -r '.registry.version // .registry_version // ""' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"
  support_owner="$(jq -r '.support_owner // .ontology_owner // .oncall_owner // ""' "$artifact")"
  archive_uri="$(jq -r '.evidence_archive.uri // .archive.uri // ""' "$artifact")"
  immutable="$(jq -r '.evidence_archive.immutable // .archive.immutable // false' "$artifact")"
  archive_digest="$(jq -r '.evidence_archive.digest // .archive.digest // ""' "$artifact")"
  retention_policy="$(jq -r '.evidence_archive.retention_policy // .archive.retention_policy // ""' "$artifact")"

  if ! ready_value "$status"; then
    printf 'ontology engine production status is not ready: %s' "$status"
    return 0
  fi
  if [[ "$evidence_class" != "customer_grade" ]]; then
    printf 'ontology engine evidence class is not customer_grade: %s' "${evidence_class:-<empty>}"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'ontology engine target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  if [[ "$target_environment" != "production" || -z "$registry_version" ]]; then
    printf 'ontology engine evidence lacks production environment or registry version'
    return 0
  fi
  case "$target_kind" in
    production_ontology_engine|ontology_engine|customer_grade_deployment|kubernetes_cluster|managed_agent_cluster) ;;
    *)
      printf 'ontology engine target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
    printf 'ontology engine evidence lacks audit, timestamp, or support owner'
    return 0
  fi
  if [[ "$immutable" != "true" || -z "$archive_uri" || -z "$archive_digest" || -z "$retention_policy" ]]; then
    printf 'ontology engine evidence lacks immutable archive URI, digest, or retention metadata'
    return 0
  fi

  jq -e '
    def ready: . == "ready" or . == "validated" or . == "completed" or . == "passed";
    (.registry.status // "unknown" | ready)
    and ((.registry.core_version // "") | length > 0)
    and ((.registry.domain_versions // []) | length > 0)
    and (.migrations.status // "unknown" | ready)
    and (.migrations.promote_tested == true)
    and (.migrations.rollback_tested == true)
    and (.migrations.forward_migration_tested == true)
    and (.migrations.migration_policy_present == true)
    and (.relation_constraints.status // "unknown" | ready)
    and (.relation_constraints.enforced_before_policy == true)
    and ((.relation_constraints.coverage // []) | index("object_type") != null)
    and ((.relation_constraints.coverage // []) | index("link_type") != null)
    and ((.relation_constraints.coverage // []) | index("cardinality") != null)
    and ((.relation_constraints.coverage // []) | index("domain_scope") != null)
    and (.conflict_trust.status // "unknown" | ready)
    and (.conflict_trust.conflict_policy_tested == true)
    and (.conflict_trust.contradiction_blocks_high_risk == true)
    and (.conflict_trust.stale_fact_blocks_high_risk == true)
    and (.conflict_trust.trust_downgrade_blocks_high_risk == true)
    and (.builder_approvals.status // "unknown" | ready)
    and (.builder_approvals.reviewable_proposals == true)
    and (.builder_approvals.approved_changes_audited == true)
    and (.context_packets.status // "unknown" | ready)
    and (.context_packets.source_refs_rendered == true)
    and (.context_packets.ontology_version_rendered == true)
    and (.context_packets.relation_expansion_rendered == true)
    and (.context_packets.trust_freshness_gates_enforced == true)
    and (.runtime_enforcement.status // "unknown" | ready)
    and (.runtime_enforcement.policy_precheck_uses_constraints == true)
  ' "$artifact" >/dev/null || {
    printf 'ontology engine production evidence is incomplete'
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
    --arg source "ontology-engine-production-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg ontology_engine_evidence_file "$ONTOLOGY_ENGINE_EVIDENCE_FILE" \
    --arg issue "$issue" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      ontology_engine_evidence_file: $ontology_engine_evidence_file,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "ontology_engine_production_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "ontology_engine_evidence_file=$ONTOLOGY_ENGINE_EVIDENCE_FILE"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "ontology engine production static gate ok"
  exit 0
fi

blocked_count=0
issue=""
if issue_text="$(ontology_issue "$ONTOLOGY_ENGINE_EVIDENCE_FILE")"; then
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

echo "ontology engine production gate ok"
