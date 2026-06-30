#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/live-connector-production-semantics-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/live-connector-production-semantics}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

connectors=(
  "tmall-top|packs/ecommerce-tmall/connectors/tmall-top.yaml"
  "taobao-open-platform|packs/ecommerce-taobao/connectors/taobao-open-platform.yaml"
  "xiaohongshu-shop|packs/ecommerce-xiaohongshu/connectors/xiaohongshu-shop.yaml"
  "xianyu-goofish|packs/ecommerce-xianyu/connectors/xianyu-goofish.yaml"
  "tiktok-shop-open-api|packs/ecommerce-tiktok-shop/connectors/tiktok-shop-open-api.yaml"
  "amazon-selling-partner-api|packs/ecommerce-amazon/connectors/amazon-selling-partner-api.yaml"
  "github-connector|packs/swe-review/connectors/github-connector.yaml"
)

enterprise_connectors=(
  "lark-mcp"
  "feishu-mcp"
  "lark-native"
  "feishu-native"
)

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "live connector production semantics gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "live connector production semantics gate failed: $*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || fail "missing required file: $1"
}

require_pattern() {
  local pattern="$1"
  local file="$2"
  local message="$3"
  grep -Eq "$pattern" "$file" || fail "$message: $file"
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
  for connector in "${connectors[@]}"; do
    local connector_id="${connector%%|*}"
    local manifest="${connector#*|}"
    require_file "$manifest"
    require_pattern 'production_readiness:' "$manifest" "$connector_id must declare production_readiness"
    require_pattern 'required_evidence_class:[[:space:]]*customer_grade' "$manifest" "$connector_id must require customer_grade evidence"
    require_pattern 'fail_closed_without_evidence:[[:space:]]*true' "$manifest" "$connector_id must fail closed without evidence"
    require_pattern 'environment_separation:' "$manifest" "$connector_id must declare environment separation"
    require_pattern 'sandbox_base_url_env:' "$manifest" "$connector_id must declare sandbox base URL env"
    require_pattern 'live_base_url_env:' "$manifest" "$connector_id must declare live base URL env"
    require_pattern 'token_lifecycle:' "$manifest" "$connector_id must declare token lifecycle"
    require_pattern 'controller_env:' "$manifest" "$connector_id must declare controller envs"
    require_pattern 'rate_limit_retry:' "$manifest" "$connector_id must declare rate limit and retry policy"
    require_pattern 'error_taxonomy:' "$manifest" "$connector_id must declare platform error taxonomy"
    require_pattern 'idempotency_reconciliation:' "$manifest" "$connector_id must declare idempotency and reconciliation"
    require_pattern 'idempotency_fields:.*operation_id.*payload_digest' "$manifest" "$connector_id must include operation and payload idempotency fields"
    require_pattern 'webhook_ingestion:' "$manifest" "$connector_id must declare webhook or polling ingestion"
    require_pattern 'compensation:' "$manifest" "$connector_id must declare compensation policy"
    require_pattern 'mode:[[:space:]]*compensation_or_explicit_non_compensable' "$manifest" "$connector_id must distinguish compensable and non-compensable operations"
    require_pattern 'approval_commit_boundary:' "$manifest" "$connector_id must declare approval commit boundary"
    require_pattern 'secret_redaction:' "$manifest" "$connector_id must declare secret redaction"
    require_pattern 'prompt_injection_boundary:' "$manifest" "$connector_id must declare prompt injection boundary"
    require_pattern 'treat_results_as_data:[[:space:]]*true' "$manifest" "$connector_id must treat connector results as data"
  done

  grep -q "GitHub SWE connector" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must require GitHub SWE connector promotion"
  grep -q "Lark/Feishu MCP and native enterprise connectors" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must require Lark/Feishu MCP and native connector promotion"
  for connector_id in "${enterprise_connectors[@]}"; do
    local artifact_path="live-connector-production-semantics/$connector_id/summary.json"
    grep -q "$artifact_path" crates/mandoforge-api/src/stage2_readiness.rs \
      || fail "Stage 2 readiness must require $connector_id production semantics evidence"
    grep -q "$artifact_path" scripts/stage2-completion-audit-gate.sh \
      || fail "Stage 2 completion audit must require $connector_id production semantics evidence"
    grep -q "$artifact_path" scripts/verify-stage2-evidence-archive.sh \
      || fail "Stage 2 archive verifier must require $connector_id production semantics evidence"
  done
}

connector_issue() {
  local connector_id="$1"
  local artifact="$2"
  local status target_id target_kind version archive_uri logs_uri support_owner immutable audit_id checked_at

  [[ -s "$artifact" ]] || {
    printf 'missing connector evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  version="$(jq -r '.connector.version // .version // ""' "$artifact")"
  archive_uri="$(jq -r '.deployment_archive.uri // .archive.uri // ""' "$artifact")"
  logs_uri="$(jq -r '.deployment_archive.logs_uri // .archive.logs_uri // ""' "$artifact")"
  support_owner="$(jq -r '.deployment_archive.support_owner // .support_owner // ""' "$artifact")"
  immutable="$(jq -r '.deployment_archive.immutable // .archive.immutable // false' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"

  if [[ "$(jq -r '.connector.id // .connector_id // ""' "$artifact")" != "$connector_id" ]]; then
    printf 'connector id does not match evidence path: %s' "$connector_id"
    return 0
  fi
  if ! ready_value "$status"; then
    printf 'connector evidence status is not ready: %s' "$status"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'connector target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  case "$target_kind" in
    production_connector|production_platform|customer_grade_connector|connector_runtime|kubernetes_cluster|managed_connector_cluster) ;;
    *)
      printf 'connector target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$version" || -z "$audit_id" || -z "$checked_at" ]]; then
    printf 'connector evidence lacks version, audit, or timestamp'
    return 0
  fi
  if [[ "$immutable" != "true" || -z "$archive_uri" || -z "$logs_uri" || -z "$support_owner" ]]; then
    printf 'connector deployment archive lacks immutable uri, logs, or support owner'
    return 0
  fi

  jq -e '
    def ready: . == "ready" or . == "validated" or . == "completed" or . == "passed";
    (.sandbox_live_separation.status // "unknown" | ready)
    and (.token_lifecycle.status // "unknown" | ready)
    and (.token_lifecycle.refresh_tested == true)
    and (.token_lifecycle.expiry_tested == true)
    and (.token_lifecycle.rotation_tested == true)
    and (.rate_limit_retry.status // "unknown" | ready)
    and ([.rate_limit_retry.error_taxonomy[]?] | length > 0)
    and (.idempotency_reconciliation.status // "unknown" | ready)
    and (.idempotency_reconciliation.idempotency_key_count // 0 > 0)
    and (.idempotency_reconciliation.external_reconciliation_tested == true)
    and (.webhook_ingestion.status // "unknown" | ready)
    and (.webhook_ingestion.provenance_captured == true)
    and (.compensation.status // "unknown" | ready)
    and ((.compensation.mode // "") == "compensation_or_explicit_non_compensable")
    and (.approval_commit_boundary.status // "unknown" | ready)
    and (.secret_redaction.status // "unknown" | ready)
    and (.secret_redaction.no_raw_secret_leakage == true)
  ' "$artifact" >/dev/null || {
    printf 'connector production semantics are incomplete in %s' "$artifact"
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
    --arg source "live-connector-production-semantics-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg source_evidence_dir "$SOURCE_EVIDENCE_DIR" \
    --arg issue "$issue" \
    --argjson connector_count "$((${#connectors[@]} + ${#enterprise_connectors[@]}))" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      source_evidence_dir: $source_evidence_dir,
      connector_count: $connector_count,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "live_connector_production_semantics_status=$status"
    echo "connector_count=$((${#connectors[@]} + ${#enterprise_connectors[@]}))"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "source_evidence_dir=$SOURCE_EVIDENCE_DIR"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "live connector production semantics static gate ok"
  exit 0
fi

blocked_count=0
issue=""
for connector in "${connectors[@]}"; do
  connector_id="${connector%%|*}"
  artifact="$SOURCE_EVIDENCE_DIR/$connector_id/summary.json"
  if issue_text="$(connector_issue "$connector_id" "$artifact")"; then
    echo "$connector_id: $issue_text" >&2
    issue="${issue:+$issue; }$connector_id: $issue_text"
    blocked_count=$((blocked_count + 1))
  fi
done
for connector_id in "${enterprise_connectors[@]}"; do
  artifact="$SOURCE_EVIDENCE_DIR/$connector_id/summary.json"
  if issue_text="$(connector_issue "$connector_id" "$artifact")"; then
    echo "$connector_id: $issue_text" >&2
    issue="${issue:+$issue; }$connector_id: $issue_text"
    blocked_count=$((blocked_count + 1))
  fi
done

if [[ "$blocked_count" == "0" ]]; then
  write_summary "ready" 0 ""
else
  write_summary "blocked" "$blocked_count" "$issue"
fi

cat "$EVIDENCE_DIR/summary.txt"

if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
  exit 1
fi

echo "live connector production semantics gate ok"
