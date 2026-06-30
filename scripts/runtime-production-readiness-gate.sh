#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/runtime-production-evidence}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
RUNTIME_RECOVERY_EVIDENCE_FILE="${RUNTIME_PRODUCTION_RECOVERY_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/runtime-production-recovery-evidence.json}"
STAGE2_ARCHIVE="${RUNTIME_PRODUCTION_STAGE2_ARCHIVE:-}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "runtime production readiness gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "runtime production readiness gate failed: $*" >&2
  exit 1
}

is_positive_integer() {
  [[ "$1" =~ ^[0-9]+$ && "$1" -gt 0 ]]
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

require_executable() {
  [[ -x "$1" ]] || fail "missing executable script: $1"
}

static_contract_check() {
  require_executable scripts/managed-session-runtime-evidence-gate.sh
  require_executable scripts/worker-evidence-gate.sh
  require_executable scripts/stage2-completion-audit-gate.sh
  require_executable scripts/verify-stage2-evidence-archive.sh
  require_executable scripts/runtime-production-readiness-gate.sh

  grep -q "runtime-production-readiness-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the runtime production gate"
  grep -q "runtime-production-readiness-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the runtime production gate"
  grep -q "runtime-production-readiness-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the runtime production gate"
  grep -q "runtime-production-readiness-gate.sh" deploy/stage2-evidence/runtime-production-evidence-job.example.yaml \
    || fail "runtime production evidence Job must run the runtime production gate"
  grep -q "runtime-production-evidence-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the runtime production evidence Job"
  grep -q "runtime-production-recovery-evidence.json" scripts/verify-stage2-evidence-k8s-manifests.sh \
    || fail "Stage 2 manifest verifier must require runtime recovery evidence"
}

runtime_recovery_issue() {
  local artifact="$1"
  local status
  local target_id
  local target_kind
  local backup_status
  local backup_resource_count
  local restore_audit
  local restore_time
  local dead_letter_status
  local dead_letter_configured
  local manual_replay_tested
  local replay_audit
  local replay_time
  local idempotency_status
  local idempotency_covered
  local idempotency_key_count

  [[ -s "$artifact" ]] || {
    printf 'missing runtime recovery evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  target_id="$(jq -r '.target.id // .target.cluster_id // .target.deployment_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  backup_status="$(jq -r '.backup_restore.status // "unknown"' "$artifact")"
  backup_resource_count="$(jq -r '[.backup_restore.preserved_resources[]?] | unique | length' "$artifact")"
  restore_audit="$(jq -r '.backup_restore.audit_id // .backup_restore.audit_log_id // .backup_restore.trace_id // .backup_restore.run_id // ""' "$artifact")"
  restore_time="$(jq -r '.backup_restore.completed_at // .backup_restore.restored_at // .backup_restore.checked_at // .backup_restore.timestamp // ""' "$artifact")"
  dead_letter_status="$(jq -r '.dead_letter_replay.status // "unknown"' "$artifact")"
  dead_letter_configured="$(jq -r '.dead_letter_replay.dead_letter_queue_configured // false' "$artifact")"
  manual_replay_tested="$(jq -r '.dead_letter_replay.manual_replay_tested // false' "$artifact")"
  replay_audit="$(jq -r '.dead_letter_replay.audit_id // .dead_letter_replay.audit_log_id // .dead_letter_replay.trace_id // .dead_letter_replay.run_id // ""' "$artifact")"
  replay_time="$(jq -r '.dead_letter_replay.completed_at // .dead_letter_replay.replayed_at // .dead_letter_replay.checked_at // .dead_letter_replay.timestamp // ""' "$artifact")"
  idempotency_status="$(jq -r '.idempotency.status // "unknown"' "$artifact")"
  idempotency_covered="$(jq -r '.idempotency.external_side_effect_idempotency_covered // false' "$artifact")"
  idempotency_key_count="$(jq -r '.idempotency.idempotency_key_count // 0' "$artifact")"

  if [[ "$status" != "validated" && "$status" != "ready" && "$status" != "completed" ]]; then
    printf 'runtime recovery status is not validated: %s' "$status"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'runtime recovery target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  case "$target_kind" in
    managed_runtime|production_runtime_cluster|managed_agent_cluster|k8s_cluster|kubernetes_cluster) ;;
    *)
      printf 'runtime recovery target kind is not production runtime: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ "$backup_status" != "validated" && "$backup_status" != "completed" && "$backup_status" != "ready" ]]; then
    printf 'backup/restore drill is not validated: %s' "$backup_status"
    return 0
  fi
  if [[ "$backup_resource_count" -lt 8 || -z "$restore_audit" || -z "$restore_time" ]]; then
    printf 'backup/restore drill lacks preserved resource, audit, or timestamp evidence'
    return 0
  fi
  if [[ "$dead_letter_status" != "validated" && "$dead_letter_status" != "completed" && "$dead_letter_status" != "ready" ]]; then
    printf 'dead-letter/manual replay drill is not validated: %s' "$dead_letter_status"
    return 0
  fi
  if [[ "$dead_letter_configured" != "true" || "$manual_replay_tested" != "true" || -z "$replay_audit" || -z "$replay_time" ]]; then
    printf 'dead-letter/manual replay lacks queue, replay, audit, or timestamp evidence'
    return 0
  fi
  if [[ "$idempotency_status" != "validated" && "$idempotency_status" != "completed" && "$idempotency_status" != "ready" ]]; then
    printf 'idempotency drill is not validated: %s' "$idempotency_status"
    return 0
  fi
  if [[ "$idempotency_covered" != "true" ]] || ! is_positive_integer "$idempotency_key_count"; then
    printf 'external side-effect idempotency evidence is incomplete'
    return 0
  fi

  return 1
}

write_summary() {
  local status="$1"
  local blocked_count="$2"
  local runtime_recovery_status="$3"
  local runtime_recovery_issue_text="$4"
  local summary_json="$EVIDENCE_DIR/summary.json"
  local summary_txt="$EVIDENCE_DIR/summary.txt"

  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg source "runtime-production-readiness-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg source_evidence_dir "$SOURCE_EVIDENCE_DIR" \
    --arg runtime_recovery_evidence_file "$RUNTIME_RECOVERY_EVIDENCE_FILE" \
    --arg runtime_recovery_status "$runtime_recovery_status" \
    --arg runtime_recovery_issue "$runtime_recovery_issue_text" \
    --arg stage2_archive "$STAGE2_ARCHIVE" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      source_evidence_dir: $source_evidence_dir,
      runtime_recovery_evidence_file: $runtime_recovery_evidence_file,
      runtime_recovery_status: $runtime_recovery_status,
      runtime_recovery_issue: (if $runtime_recovery_issue == "" then null else $runtime_recovery_issue end),
      stage2_archive: (if $stage2_archive == "" then null else $stage2_archive end),
      blocked_count: $blocked_count
    }' >"$summary_json"

  {
    echo "runtime_production_status=$status"
    echo "blocked_count=$blocked_count"
    echo "runtime_recovery_status=$runtime_recovery_status"
    if [[ -n "$runtime_recovery_issue_text" ]]; then
      echo "runtime_recovery_issue=$runtime_recovery_issue_text"
    fi
    echo "source_evidence_dir=$SOURCE_EVIDENCE_DIR"
    echo "runtime_recovery_evidence_file=$RUNTIME_RECOVERY_EVIDENCE_FILE"
  } >"$summary_txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 "not_checked" ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "runtime production static readiness gate ok"
  exit 0
fi

blocked_count=0
runtime_recovery_status="ready"
runtime_recovery_issue_text=""

if [[ -n "$STAGE2_ARCHIVE" ]]; then
  scripts/verify-stage2-evidence-archive.sh "$STAGE2_ARCHIVE" >/dev/null
fi

if [[ -s "$SOURCE_EVIDENCE_DIR/managed-session-restart-resume-evidence.json" ]]; then
  MANAGED_SESSION_RESTART_RESUME_EVIDENCE_FILE="$SOURCE_EVIDENCE_DIR/managed-session-restart-resume-evidence.json" \
    BASE_URL="$BASE_URL" \
    EVIDENCE_DIR="$EVIDENCE_DIR/managed-session-runtime" \
    ALLOW_BLOCKED=0 \
    scripts/managed-session-runtime-evidence-gate.sh >/dev/null
else
  echo "missing managed-session restart/resume evidence: $SOURCE_EVIDENCE_DIR/managed-session-restart-resume-evidence.json" >&2
  blocked_count=$((blocked_count + 1))
fi

if [[ -s "$SOURCE_EVIDENCE_DIR/worker/summary.txt" ]]; then
  if grep -q '^production_blocked_count=0$' "$SOURCE_EVIDENCE_DIR/worker/summary.txt"; then
    :
  else
    echo "worker evidence summary is blocked: $SOURCE_EVIDENCE_DIR/worker/summary.txt" >&2
    blocked_count=$((blocked_count + 1))
  fi
else
  echo "missing worker evidence summary: $SOURCE_EVIDENCE_DIR/worker/summary.txt" >&2
  blocked_count=$((blocked_count + 1))
fi

if runtime_recovery_issue_text="$(runtime_recovery_issue "$RUNTIME_RECOVERY_EVIDENCE_FILE")"; then
  runtime_recovery_status="blocked"
  echo "$runtime_recovery_issue_text" >&2
  blocked_count=$((blocked_count + 1))
fi

if [[ "$blocked_count" == "0" ]]; then
  write_summary "ready" "$blocked_count" "$runtime_recovery_status" ""
else
  write_summary "blocked" "$blocked_count" "$runtime_recovery_status" "$runtime_recovery_issue_text"
fi

cat "$EVIDENCE_DIR/summary.txt"

if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
  exit 1
fi

echo "runtime production readiness gate ok"
