#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-tenant-isolation-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/tenant-isolation-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-}"

auth_headers=(
)
if [[ -n "$AUTH_TOKEN" ]]; then
  auth_headers+=(-H "authorization: Bearer $AUTH_TOKEN")
else
  auth_headers+=(
    -H "x-mandoforge-subject: $SUBJECT"
    -H "x-mandoforge-roles: $ROLES"
  )
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "tenant isolation evidence gate requires $1" >&2
    exit 1
  fi
}

is_multi_tenant_target_kind() {
  local value="$1"
  case "$value" in
    multi_tenant_deployment|enterprise_multi_tenant|production_multi_tenant)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

has_multiple_tenants() {
  local value="$1"
  [[ "$value" =~ ^[0-9]+$ && "$value" -ge 2 ]]
}

slugify() {
  printf '%s' "$1" | sed -E 's#^/##; s#[/:]+#-#g; s#[^A-Za-z0-9._-]+#-#g'
}

fetch_json() {
  local method="$1"
  local path="$2"
  local label
  label="$(slugify "$path")"
  local target="$EVIDENCE_DIR/$label.json"
  local response_body
  local http_status
  response_body="$(mktemp)"

  if [[ "$method" == "GET" ]]; then
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" "${auth_headers[@]}" "$BASE_URL$path")"
  else
    http_status="$(curl -sS -o "$response_body" -w "%{http_code}" -X "$method" "${auth_headers[@]}" \
      -H "content-type: application/json" \
      -d '{}' \
      "$BASE_URL$path")"
  fi

  if [[ "$http_status" != 2* ]]; then
    echo "tenant isolation evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-tenant-isolation-readiness.json"
  local validation_evidence_file="$EVIDENCE_DIR/tenant-routing-validation-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local score
  local runtime_mode
  local routing_status
  local cross_tenant_routing_supported
  local routing_controller_fresh
  local routing_controller_age_hours
  local routing_validation_evidence_status
  local routing_validation_status
  local routing_target_kind
  local routing_environment
  local routing_tenant_count
  local routing_rls_enforced
  local routing_tenant_context_validated
  local routing_cross_tenant_negative_tests
  local rls_enabled
  local rls_forced
  local tenant_context
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  score="$(jq -r '.readiness_score // 0' "$readiness_file")"
  runtime_mode="$(jq -r '.runtime_tenant_mode // "unknown"' "$readiness_file")"
  routing_status="$(jq -r '.production_routing.status // "unknown"' "$readiness_file")"
  cross_tenant_routing_supported="$(jq -r '.production_routing.cross_tenant_routing_supported // false' "$readiness_file")"
  routing_controller_fresh="$(jq -r '.production_routing.controller_evidence_fresh // false' "$readiness_file")"
  routing_controller_age_hours="$(jq -r '.production_routing.latest_controller_age_hours // "none"' "$readiness_file")"
  routing_validation_evidence_status="missing"
  routing_validation_status="unknown"
  routing_target_kind="unknown"
  routing_environment="unknown"
  routing_tenant_count="0"
  routing_rls_enforced="false"
  routing_tenant_context_validated="false"
  routing_cross_tenant_negative_tests="false"
  if [[ -s "$validation_evidence_file" ]]; then
    routing_validation_evidence_status="$(jq -r '.status // "unknown"' "$validation_evidence_file")"
    routing_validation_status="$(jq -r '.response.status // "unknown"' "$validation_evidence_file")"
    routing_target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$validation_evidence_file")"
    routing_environment="$(jq -r '.response.controller_execution.environment // "unknown"' "$validation_evidence_file")"
    routing_tenant_count="$(jq -r '.response.controller_execution.tenant_count // 0' "$validation_evidence_file")"
    routing_rls_enforced="$(jq -r '.response.controller_execution.rls_enforced // false' "$validation_evidence_file")"
    routing_tenant_context_validated="$(jq -r '.response.controller_execution.tenant_context_validated // false' "$validation_evidence_file")"
    routing_cross_tenant_negative_tests="$(jq -r '.response.controller_execution.cross_tenant_negative_tests // false' "$validation_evidence_file")"
  fi
  rls_enabled="$(jq -r '.rls.enabled // false' "$readiness_file")"
  rls_forced="$(jq -r '.rls.forced // false' "$readiness_file")"
  tenant_context="$(jq -r '.rls.tenant_context_configured // false' "$readiness_file")"
  blocked_count="$(jq -r '[
      .production_routing.production_blocked,
      (.runtime_tenant_mode != "tenant_routed"),
      (.production_routing.cross_tenant_routing_supported != true),
      (.production_routing.controller_evidence_fresh != true),
      (.rls.enabled != true),
      (.rls.forced != true),
      (.rls.tenant_context_configured != true)
    ] | map(select(. == true)) | length' "$readiness_file")"
  if [[ "$routing_validation_evidence_status" != "captured" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$routing_validation_status" != "validated" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_multi_tenant_target_kind "$routing_target_kind"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! has_multiple_tenants "$routing_tenant_count"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$routing_rls_enforced" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$routing_tenant_context_validated" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$routing_cross_tenant_negative_tests" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi

  {
    echo "tenant_isolation_status=$status"
    echo "readiness_score=$score"
    echo "runtime_tenant_mode=$runtime_mode"
    echo "production_routing_status=$routing_status"
    echo "cross_tenant_routing_supported=$cross_tenant_routing_supported"
    echo "production_routing_controller_evidence_fresh=$routing_controller_fresh"
    echo "production_routing_controller_age_hours=$routing_controller_age_hours"
    echo "routing_validation_evidence_status=$routing_validation_evidence_status"
    echo "routing_validation_status=$routing_validation_status"
    echo "routing_target_kind=$routing_target_kind"
    echo "routing_environment=$routing_environment"
    echo "routing_tenant_count=$routing_tenant_count"
    echo "routing_rls_enforced=$routing_rls_enforced"
    echo "routing_tenant_context_validated=$routing_tenant_context_validated"
    echo "routing_cross_tenant_negative_tests=$routing_cross_tenant_negative_tests"
    echo "rls_enabled=$rls_enabled"
    echo "rls_forced=$rls_forced"
    echo "tenant_context_configured=$tenant_context"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "routing_blocking_reasons:"
    jq -r '.production_routing.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    if [[ "$runtime_mode" != "tenant_routed" ]]; then
      echo "- runtime tenant mode is not tenant_routed: $runtime_mode"
    fi
    if [[ "$cross_tenant_routing_supported" != "true" ]]; then
      echo "- cross-tenant routing support is not enabled"
    fi
    if [[ "$routing_controller_fresh" != "true" ]]; then
      echo "- tenant routing controller evidence is not fresh"
    fi
    if [[ "$routing_validation_status" != "validated" ]]; then
      echo "- tenant routing validation status is not validated: $routing_validation_status"
    fi
    if ! is_multi_tenant_target_kind "$routing_target_kind"; then
      echo "- tenant routing controller target is not a broader multi-tenant deployment: $routing_target_kind"
    fi
    if ! has_multiple_tenants "$routing_tenant_count"; then
      echo "- tenant routing controller did not report multiple tenants: tenant_count=$routing_tenant_count"
    fi
    if [[ "$routing_rls_enforced" != "true" ]]; then
      echo "- tenant routing controller did not confirm RLS enforcement"
    fi
    if [[ "$routing_tenant_context_validated" != "true" ]]; then
      echo "- tenant routing controller did not confirm tenant context propagation"
    fi
    if [[ "$routing_cross_tenant_negative_tests" != "true" ]]; then
      echo "- tenant routing controller did not confirm cross-tenant negative tests"
    fi
    if [[ "$rls_enabled" != "true" || "$rls_forced" != "true" || "$tenant_context" != "true" ]]; then
      echo "- RLS is not fully enabled, forced, and tenant-context configured"
    fi
    echo
    echo "runbook_actions:"
    jq -r '.runbook_actions[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Tenant isolation evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_routing_validation_evidence() {
  local routing_response
  local validation_evidence_file="$EVIDENCE_DIR/tenant-routing-validation-evidence.json"

  routing_response="$(fetch_json POST /api/tenant-isolation/routing/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$routing_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$routing_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$validation_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/tenant-isolation/readiness >/dev/null
capture_routing_validation_evidence
fetch_json GET /api/tenant-isolation/readiness >/dev/null
write_summary
