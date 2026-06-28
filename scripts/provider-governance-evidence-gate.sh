#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-provider-governance-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/provider-governance-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_PROVIDER_ROLLOUT="${RUN_STAGE2_PROVIDER_ROLLOUT:-0}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-${MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN:-}}}"

auth_headers=()
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
    echo "provider governance evidence gate requires $1" >&2
    exit 1
  fi
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
    echo "provider governance evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local providers_json="$EVIDENCE_DIR/api-providers.json"
  local summary_json="$EVIDENCE_DIR/api-providers-summary.json"
  local gate_json="$EVIDENCE_DIR/api-providers-policy-gate.json"
  local runs_json="$EVIDENCE_DIR/api-providers-policy-gate-runs.json"
  local provider_health_json="$EVIDENCE_DIR/api-providers-health.json"
  local runtime_json="$EVIDENCE_DIR/api-providers-runtime.json"
  local rollout_evidence_file="$EVIDENCE_DIR/provider-production-rollout-evidence.json"
  local rollback_evidence_file="$EVIDENCE_DIR/provider-production-rollback-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local provider_count
  local active_count
  local deployment_status
  local deployment_controller_fresh
  local deployment_controller_age_hours
  local gate_status
  local gate_enforcement_status
  local provider_name
  local provider_type
  local provider_health_status
  local provider_health_external_probe
  local provider_health_api_key_env_present
  local provider_health_api_key_ref_resolved
  local provider_runtime_mode
  local provider_runtime_production_mode
  local provider_runtime_contract
  local active_mock_provider_count
  local rollout_evidence_status
  local rollout_run_status
  local rollback_evidence_status
  local rollback_run_status
  local blocked_count

  provider_count="$(jq -r '.provider_count // 0' "$summary_json")"
  active_count="$(jq -r '.active_provider_count // 0' "$summary_json")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$summary_json")"
  deployment_controller_fresh="$(jq -r '.deployment_readiness.controller_evidence_fresh // false' "$summary_json")"
  deployment_controller_age_hours="$(jq -r '.deployment_readiness.latest_controller_age_hours // "none"' "$summary_json")"
  gate_status="$(jq -r '.status // "unknown"' "$gate_json")"
  gate_enforcement_status="$(jq -r '.production_enforcement.status // "unknown"' "$runs_json")"
  provider_name="$(jq -r 'map(select(.status == "active")) | .[0].name // .[0].name // "none"' "$providers_json")"
  provider_type="$(jq -r 'map(select(.status == "active")) | .[0].provider_type // .[0].provider_type // "none"' "$providers_json")"
  provider_health_status="$(jq -r '.status // "unknown"' "$provider_health_json")"
  provider_health_external_probe="$(jq -r '.checks.external_probe // "unknown"' "$provider_health_json")"
  provider_health_api_key_env_present="$(jq -r '.checks.api_key_env_present // "unknown"' "$provider_health_json")"
  provider_health_api_key_ref_resolved="$(jq -r '.checks.api_key_ref_resolved // "unknown"' "$provider_health_json")"
  provider_runtime_mode="$(jq -r '.mode // "unknown"' "$runtime_json")"
  provider_runtime_production_mode="$(jq -r '.production_mode // false' "$runtime_json")"
  provider_runtime_contract="$(jq -r '.contract // "unknown"' "$runtime_json")"
  active_mock_provider_count="$(jq -r '[.[]? | select(.status == "active" and ((.provider_type // "" | ascii_downcase | gsub("-"; "_")) == "mock" or (.provider_type // "" | ascii_downcase | gsub("-"; "_")) == "mock_openai_compatible"))] | length' "$providers_json")"
  rollout_evidence_status="not_requested"
  rollout_run_status="not_run"
  if [[ -s "$rollout_evidence_file" ]]; then
    rollout_evidence_status="$(jq -r '.status // "unknown"' "$rollout_evidence_file")"
    rollout_run_status="$(jq -r '.response.status // "unknown"' "$rollout_evidence_file")"
  fi
  rollback_evidence_status="not_requested"
  rollback_run_status="not_run"
  if [[ -s "$rollback_evidence_file" ]]; then
    rollback_evidence_status="$(jq -r '.status // "unknown"' "$rollback_evidence_file")"
    rollback_run_status="$(jq -r '.response.status // "unknown"' "$rollback_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .deployment_readiness.production_blocked
    ] | map(select(. == true)) | length' "$summary_json")"
  blocked_count="$((blocked_count + $(jq -r 'if .production_enforcement.production_blocked == true then 1 else 0 end' "$runs_json")))"
  blocked_count="$((blocked_count + active_mock_provider_count))"
  if [[ "$provider_runtime_production_mode" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi

  {
    echo "provider_count=$provider_count"
    echo "active_provider_count=$active_count"
    echo "provider_policy_gate_status=$gate_status"
    echo "provider_policy_gate_enforcement_status=$gate_enforcement_status"
    echo "provider_name=$provider_name"
    echo "provider_type=$provider_type"
    echo "provider_health_status=$provider_health_status"
    echo "provider_health_external_probe=$provider_health_external_probe"
    echo "provider_health_api_key_env_present=$provider_health_api_key_env_present"
    echo "provider_health_api_key_ref_resolved=$provider_health_api_key_ref_resolved"
    echo "provider_runtime_mode=$provider_runtime_mode"
    echo "provider_runtime_production_mode=$provider_runtime_production_mode"
    echo "provider_runtime_contract=$provider_runtime_contract"
    echo "active_mock_provider_count=$active_mock_provider_count"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_controller_evidence_fresh=$deployment_controller_fresh"
    echo "deployment_controller_age_hours=$deployment_controller_age_hours"
    echo "provider_rollout_evidence_status=$rollout_evidence_status"
    echo "provider_rollout_run_status=$rollout_run_status"
    echo "provider_rollback_evidence_status=$rollback_evidence_status"
    echo "provider_rollback_run_status=$rollback_run_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "provider_rollout_run=$RUN_PROVIDER_ROLLOUT"
    echo
    echo "governance_attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$summary_json"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$summary_json"
    echo
    echo "policy_gate_attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$runs_json"
    if [[ "$active_mock_provider_count" != "0" ]]; then
      echo
      echo "provider_runtime_blocking_reasons:"
      jq -r '.[]? | select(.status == "active" and ((.provider_type // "" | ascii_downcase) == "mock" or (.provider_type // "" | ascii_downcase) == "mock_openai_compatible")) | "- active provider uses mock runtime: \(.name)"' "$providers_json"
    fi
    if [[ "$provider_runtime_production_mode" != "true" ]]; then
      echo
      echo "provider_runtime_blocking_reasons:"
      echo "- provider runtime target is not in production mode"
    fi
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Provider governance evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_provider_rollout_evidence() {
  local rollout_response
  local rollout_evidence_file="$EVIDENCE_DIR/provider-production-rollout-evidence.json"

  rollout_response="$(fetch_json POST /api/providers/production-rollout/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$rollout_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rollout_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$rollout_evidence_file"
}

capture_provider_rollback_evidence() {
  local rollback_response
  local rollback_evidence_file="$EVIDENCE_DIR/provider-production-rollback-evidence.json"

  rollback_response="$(fetch_json POST /api/providers/production-rollout/rollback)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$rollback_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$rollback_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$rollback_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
providers_file="$(fetch_json GET /api/providers)"
fetch_json GET /api/providers/runtime >/dev/null
fetch_json GET /api/providers/summary >/dev/null
fetch_json GET /api/providers/policy-gate >/dev/null
fetch_json GET /api/providers/policy-gate/runs >/dev/null
fetch_json POST /api/providers/policy-gate/run >/dev/null
fetch_json POST /api/providers/deployment/validate >/dev/null
provider_id="$(jq -r 'map(select(.status == "active")) | .[0].id // .[0].id // empty' "$providers_file")"
if [[ -n "$provider_id" ]]; then
  provider_health_file="$(fetch_json GET "/api/providers/$provider_id/health")"
  cp "$provider_health_file" "$EVIDENCE_DIR/api-providers-health.json"
else
  jq -n '{status: "missing", checks: {external_probe: "not_run", api_key_env_present: "unknown", api_key_ref_resolved: "unknown"}}' >"$EVIDENCE_DIR/api-providers-health.json"
fi

if [[ "$RUN_PROVIDER_ROLLOUT" == "1" ]]; then
  capture_provider_rollout_evidence
  capture_provider_rollback_evidence
else
  echo "skipping provider production rollout/rollback; set RUN_STAGE2_PROVIDER_ROLLOUT=1 to include provider rollout evidence" >&2
fi

fetch_json GET /api/providers/summary >/dev/null
fetch_json GET /api/providers/policy-gate >/dev/null
fetch_json GET /api/providers/policy-gate/runs >/dev/null
write_summary
