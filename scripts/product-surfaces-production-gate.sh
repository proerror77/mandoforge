#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/product-surfaces-production-gate}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
PRODUCT_SURFACES_EVIDENCE_FILE="${PRODUCT_SURFACES_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/product-surfaces/summary.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "product surfaces production gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "product surfaces production gate failed: $*" >&2
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
  require_executable scripts/verify-static-ui-assets.sh
  require_executable scripts/verify-static-ui-actionbook.sh
  require_executable scripts/product-surfaces-production-gate.sh
  [[ -s scripts/verify-ui-api-truth-gate.mjs ]] || fail "missing UI API truth gate"
  node scripts/verify-ui-api-truth-gate.mjs

  grep -q "product-surfaces-production-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the product surfaces production gate"
  grep -q "product-surfaces-production-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the product surfaces production gate"
  grep -q "product-surfaces-production-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the product surfaces production gate"
  grep -q "product-surfaces-production-gate.sh" deploy/stage2-evidence/product-surfaces-production-job.example.yaml \
    || fail "product surfaces production Job must run the dedicated gate"
  grep -q "product-surfaces-production-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the product surfaces production Job"

  for route in \
    "/api/enterprise-product/readiness" \
    "/api/enterprise-security/admin-readiness" \
    "/api/native-connectors/production-readiness" \
    "/api/ontology/engine-readiness" \
    "/api/remote-computers/production-path" \
    "/api/usage/finance-operations/summary"; do
    grep -R -q "$route" web-ui/src scripts/verify-ui-api-truth-gate.mjs \
      || fail "UI production surfaces must reference live API route: $route"
  done

  if grep -R -E "\"[^\"]*(customer-grade ready|production ready)|'[^']*(customer-grade ready|production ready)" web-ui/src \
    | grep -v "/api/enterprise-product/readiness" >/dev/null; then
    fail "UI must not hard-code production completion claims outside API readbacks"
  fi
}

surface_issue() {
  local artifact="$1"
  local status evidence_class target_id target_kind target_environment target_base_url target_git_sha target_image_tag
  local audit_id checked_at support_owner archive_uri immutable archive_digest retention_policy

  [[ -s "$artifact" ]] || {
    printf 'missing product surfaces evidence artifact: %s' "$artifact"
    return 0
  }

  status="$(jq -r '.status // "unknown"' "$artifact")"
  evidence_class="$(jq -r '.evidence_class // .required_evidence_class // ""' "$artifact")"
  target_id="$(jq -r '.target.id // .target.deployment_id // .target.cluster_id // ""' "$artifact")"
  target_kind="$(jq -r '.target.kind // "unknown"' "$artifact")"
  target_environment="$(jq -r '.target.environment // ""' "$artifact")"
  target_base_url="$(jq -r '.target.base_url // ""' "$artifact")"
  target_git_sha="$(jq -r '.target.git_sha // ""' "$artifact")"
  target_image_tag="$(jq -r '.target.image_tag // ""' "$artifact")"
  audit_id="$(jq -r '.audit_id // .audit_log_id // .trace_id // .run_id // ""' "$artifact")"
  checked_at="$(jq -r '.checked_at // .validated_at // .completed_at // .timestamp // ""' "$artifact")"
  support_owner="$(jq -r '.support_owner // .product_owner // .oncall_owner // ""' "$artifact")"
  archive_uri="$(jq -r '.evidence_archive.uri // .archive.uri // ""' "$artifact")"
  immutable="$(jq -r '.evidence_archive.immutable // .archive.immutable // false' "$artifact")"
  archive_digest="$(jq -r '.evidence_archive.digest // .archive.digest // ""' "$artifact")"
  retention_policy="$(jq -r '.evidence_archive.retention_policy // .archive.retention_policy // ""' "$artifact")"

  if ! ready_value "$status"; then
    printf 'product surfaces status is not ready: %s' "$status"
    return 0
  fi
  if [[ "$evidence_class" != "customer_grade" ]]; then
    printf 'product surfaces evidence class is not customer_grade: %s' "${evidence_class:-<empty>}"
    return 0
  fi
  if ! is_production_identity "$target_id"; then
    printf 'product surfaces target id is not production-grade: %s' "${target_id:-<empty>}"
    return 0
  fi
  if [[ "$target_environment" != "production" ]] \
    || ! is_production_identity "$target_base_url" \
    || [[ -z "$target_git_sha" || -z "$target_image_tag" ]]; then
    printf 'product surfaces target lacks production environment, base URL, git SHA, or image tag'
    return 0
  fi
  case "$target_kind" in
    production_product_surface|production_ui|customer_grade_deployment|kubernetes_cluster|managed_agent_cluster) ;;
    *)
      printf 'product surfaces target kind is not production-grade: %s' "$target_kind"
      return 0
      ;;
  esac
  if [[ -z "$audit_id" || -z "$checked_at" || -z "$support_owner" ]]; then
    printf 'product surfaces evidence lacks audit, timestamp, or support owner'
    return 0
  fi
  if [[ "$immutable" != "true" || -z "$archive_uri" || -z "$archive_digest" || -z "$retention_policy" ]]; then
    printf 'product surfaces evidence lacks immutable archive URI, digest, or retention metadata'
    return 0
  fi

  jq -e '
    def ready: . == "ready" or . == "validated" or . == "completed" or . == "passed";
    def surface($id): first(.surfaces[]? | select(.id == $id));
    def has_features($id; $features):
      (($features - ((surface($id).features // []) | unique)) | length) == 0;
    def routes_checked($id):
      ((surface($id).routes // []) | length) > 0
      and all(surface($id).routes[]?;
        (.method // "") != ""
        and (.path // "" | startswith("/api/"))
        and ((.status // 0) >= 200 and (.status // 0) < 300)
        and (.schema_checked == true)
      );
    (try ((.freshness.expires_at // "" | fromdateiso8601) > now) catch false)
    and
    (surface("admin-console").status // "unknown" | ready)
    and (surface("admin-console").live_api_readback == true)
    and (surface("admin-console").authorization_boundaries_checked == true)
    and (surface("admin-console").no_fake_completion_state == true)
    and routes_checked("admin-console")
    and has_features("admin-console"; ["tenants","teams","agents","runtime_profiles","providers","policies","approvals","connectors","budgets","release_state"])
    and (surface("operator-console").status // "unknown" | ready)
    and (surface("operator-console").live_api_readback == true)
    and (surface("operator-console").authorization_boundaries_checked == true)
    and (surface("operator-console").no_fake_completion_state == true)
    and routes_checked("operator-console")
    and has_features("operator-console"; ["blocked_work","approvals","runs","replay","artifacts","execution_jobs","session_loop_jobs","manual_repair"])
    and (surface("builder-console").status // "unknown" | ready)
    and (surface("builder-console").live_api_readback == true)
    and (surface("builder-console").authorization_boundaries_checked == true)
    and (surface("builder-console").no_fake_completion_state == true)
    and routes_checked("builder-console")
    and has_features("builder-console"; ["workflow_packs","ontology_builder","connector_mapping","eval_gates","release_gates"])
    and (surface("ops-console").status // "unknown" | ready)
    and (surface("ops-console").live_api_readback == true)
    and (surface("ops-console").authorization_boundaries_checked == true)
    and (surface("ops-console").no_fake_completion_state == true)
    and routes_checked("ops-console")
    and has_features("ops-console"; ["health","workers","queues","costs","alerts","deployments","incident_evidence"])
    and (.live_api_truth.status // "unknown" | ready)
    and (.live_api_truth.route_coverage_tested == true)
    and (.live_api_truth.live_endpoint_coverage_tested == true)
    and (.live_api_truth.backend_authorization_checked == true)
    and (.live_api_truth.unauthenticated_rejected == true)
    and (.live_api_truth.forbidden_role_rejected == true)
    and (.live_api_truth.fake_completion_scan_passed == true)
    and (.live_api_truth.stale_or_mock_data_scan_passed == true)
  ' "$artifact" >/dev/null || {
    printf 'product surfaces evidence is incomplete in %s' "$artifact"
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
    --arg source "product-surfaces-production-gate" \
    --arg status "$status" \
    --arg required_evidence_class "customer_grade" \
    --arg product_surfaces_evidence_file "$PRODUCT_SURFACES_EVIDENCE_FILE" \
    --arg issue "$issue" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      source: $source,
      status: $status,
      required_evidence_class: $required_evidence_class,
      product_surfaces_evidence_file: $product_surfaces_evidence_file,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end)
    }' >"$EVIDENCE_DIR/summary.json"
  {
    echo "product_surfaces_production_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "product_surfaces_evidence_file=$PRODUCT_SURFACES_EVIDENCE_FILE"
  } >"$EVIDENCE_DIR/summary.txt"
}

require_cmd jq
require_cmd node
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "product surfaces production static gate ok"
  exit 0
fi

blocked_count=0
issue=""
if issue_text="$(surface_issue "$PRODUCT_SURFACES_EVIDENCE_FILE")"; then
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

echo "product surfaces production gate ok"
