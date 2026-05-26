#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-observability-collector-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/observability-collector-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_REMEDIATION="${RUN_STAGE2_OBSERVABILITY_REMEDIATION:-0}"
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
    echo "observability collector evidence gate requires $1" >&2
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
    echo "observability collector evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-observability-collector-readiness.json"
  local deployment_evidence_file="$EVIDENCE_DIR/observability-collector-deployment-evidence.json"
  local cluster_evidence_file="$EVIDENCE_DIR/observability-collector-cluster-rollout-evidence.json"
  local remediation_evidence_file="$EVIDENCE_DIR/observability-collector-remediation-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local production_ops_status
  local deployment_status
  local cluster_status
  local remediation_status
  local deployment_controller_fresh
  local deployment_controller_age_hours
  local deployment_evidence_status
  local deployment_validation_status
  local cluster_controller_fresh
  local cluster_controller_age_hours
  local cluster_evidence_status
  local cluster_validation_status
  local remediation_evidence_status
  local remediation_run_status
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$readiness_file")"
  deployment_status="$(jq -r '.deployment_readiness.status // "unknown"' "$readiness_file")"
  cluster_status="$(jq -r '.cluster_rollout.status // "unknown"' "$readiness_file")"
  remediation_status="$(jq -r '.remediation_supervision.status // "unknown"' "$readiness_file")"
  deployment_controller_fresh="$(jq -r '.deployment_readiness.controller_evidence_fresh // false' "$readiness_file")"
  deployment_controller_age_hours="$(jq -r '.deployment_readiness.latest_controller_age_hours // "none"' "$readiness_file")"
  cluster_controller_fresh="$(jq -r '.cluster_rollout.controller_evidence_fresh // false' "$readiness_file")"
  cluster_controller_age_hours="$(jq -r '.cluster_rollout.latest_controller_age_hours // "none"' "$readiness_file")"
  deployment_evidence_status="missing"
  deployment_validation_status="unknown"
  if [[ -s "$deployment_evidence_file" ]]; then
    deployment_evidence_status="$(jq -r '.status // "unknown"' "$deployment_evidence_file")"
    deployment_validation_status="$(jq -r '.response.status // "unknown"' "$deployment_evidence_file")"
  fi
  cluster_evidence_status="missing"
  cluster_validation_status="unknown"
  if [[ -s "$cluster_evidence_file" ]]; then
    cluster_evidence_status="$(jq -r '.status // "unknown"' "$cluster_evidence_file")"
    cluster_validation_status="$(jq -r '.response.status // "unknown"' "$cluster_evidence_file")"
  fi
  remediation_evidence_status="not_requested"
  remediation_run_status="not_run"
  if [[ -s "$remediation_evidence_file" ]]; then
    remediation_evidence_status="$(jq -r '.status // "unknown"' "$remediation_evidence_file")"
    remediation_run_status="$(jq -r '.response.status // "unknown"' "$remediation_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .production_ops.production_blocked,
      .deployment_readiness.production_blocked,
      .cluster_rollout.production_blocked,
      .remediation_supervision.production_blocked
    ] | map(select(. == true)) | length' "$readiness_file")"

  {
    echo "observability_collector_status=$status"
    echo "production_ops_status=$production_ops_status"
    echo "deployment_readiness_status=$deployment_status"
    echo "deployment_controller_evidence_fresh=$deployment_controller_fresh"
    echo "deployment_controller_age_hours=$deployment_controller_age_hours"
    echo "cluster_rollout_status=$cluster_status"
    echo "cluster_controller_evidence_fresh=$cluster_controller_fresh"
    echo "cluster_controller_age_hours=$cluster_controller_age_hours"
    echo "remediation_supervision_status=$remediation_status"
    echo "deployment_evidence_status=$deployment_evidence_status"
    echo "deployment_validation_status=$deployment_validation_status"
    echo "cluster_evidence_status=$cluster_evidence_status"
    echo "cluster_validation_status=$cluster_validation_status"
    echo "remediation_evidence_status=$remediation_evidence_status"
    echo "remediation_run_status=$remediation_run_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "remediation_run=$RUN_REMEDIATION"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "deployment_blocking_reasons:"
    jq -r '.deployment_readiness.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "cluster_blocking_reasons:"
    jq -r '.cluster_rollout.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    echo
    echo "remediation_blocking_reasons:"
    jq -r '.remediation_supervision.blocking_reasons[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Observability collector evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_deployment_evidence() {
  local deployment_response
  local deployment_evidence_file="$EVIDENCE_DIR/observability-collector-deployment-evidence.json"

  deployment_response="$(fetch_json POST /api/observability/collector/deployment/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$deployment_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$deployment_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$deployment_evidence_file"
}

capture_cluster_rollout_evidence() {
  local cluster_response
  local cluster_evidence_file="$EVIDENCE_DIR/observability-collector-cluster-rollout-evidence.json"

  cluster_response="$(fetch_json POST /api/observability/collector/cluster/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$cluster_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$cluster_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$cluster_evidence_file"
}

capture_remediation_evidence() {
  local remediation_response
  local remediation_evidence_file="$EVIDENCE_DIR/observability-collector-remediation-evidence.json"

  remediation_response="$(fetch_json POST /api/observability/remediation/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$remediation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$remediation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$remediation_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/observability >/dev/null
fetch_json GET /api/observability/collector-readiness >/dev/null
capture_deployment_evidence
capture_cluster_rollout_evidence

if [[ "$RUN_REMEDIATION" == "1" ]]; then
  capture_remediation_evidence
else
  echo "skipping observability remediation run; set RUN_STAGE2_OBSERVABILITY_REMEDIATION=1 to include remediation evidence" >&2
fi

fetch_json GET /api/observability/collector-readiness >/dev/null
write_summary
