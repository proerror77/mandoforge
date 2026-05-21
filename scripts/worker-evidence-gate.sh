#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-worker-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/worker-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
AUTH_TOKEN="${MANDOFORGE_STAGE2_GATE_TOKEN:-}"
PRODUCTION_CLUSTER_ID="${MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID:-}"

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
    echo "worker evidence gate requires $1" >&2
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
    echo "worker evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

is_real_cluster_kind() {
  local value="$1"
  case "$value" in
    k8s_cluster|kubernetes_cluster|production_cluster|real_cluster)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_multi_node() {
  local value="$1"
  [[ "$value" =~ ^[0-9]+$ && "$value" -ge 2 ]]
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-execution-jobs-worker-readiness.json"
  local load_validation_evidence_file="$EVIDENCE_DIR/worker-load-validation-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local readiness_score
  local queue_backend
  local queue_durable
  local worker_mode
  local k8s_hardening
  local autoscaling_status
  local load_validated
  local load_validation_evidence_status
  local load_validation_run_status
  local isolated_pool
  local load_validation_controller_fresh
  local load_validation_controller_age_hours
  local production_ops_status
  local blocked_count
  local load_validation_controller_status
  local load_validation_target_kind
  local load_validation_node_count
  local load_validation_cluster_id
  local load_validation_cluster_profile
  local load_validation_controller_load_validated
  local load_validation_controller_isolated_pool

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  readiness_score="$(jq -r '.readiness_score // 0' "$readiness_file")"
  queue_backend="$(jq -r '.queue_backend.kind // "unknown"' "$readiness_file")"
  queue_durable="$(jq -r '.queue_backend.durable // false' "$readiness_file")"
  worker_mode="$(jq -r '.worker_mode.mode // "unknown"' "$readiness_file")"
  k8s_hardening="$(jq -r '.k8s.hardening_status // "unknown"' "$readiness_file")"
  autoscaling_status="$(jq -r '.autoscaling.validation_status // "unknown"' "$readiness_file")"
  load_validated="$(jq -r '.load_validation.load_validated // false' "$readiness_file")"
  load_validation_evidence_status="missing"
  load_validation_run_status="unknown"
  load_validation_controller_status="missing"
  load_validation_target_kind="unknown"
  load_validation_node_count="0"
  load_validation_cluster_id=""
  load_validation_cluster_profile="unknown"
  load_validation_controller_load_validated="false"
  load_validation_controller_isolated_pool="false"
  if [[ -s "$load_validation_evidence_file" ]]; then
    load_validation_evidence_status="$(jq -r '.status // "unknown"' "$load_validation_evidence_file")"
    load_validation_run_status="$(jq -r '.response.status // "unknown"' "$load_validation_evidence_file")"
    load_validation_controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$load_validation_evidence_file")"
    load_validation_target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$load_validation_evidence_file")"
    load_validation_node_count="$(jq -r '.response.controller_execution.node_count // 0' "$load_validation_evidence_file")"
    load_validation_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$load_validation_evidence_file")"
    load_validation_cluster_profile="$(jq -r '.response.controller_execution.cluster_profile // "unknown"' "$load_validation_evidence_file")"
    load_validation_controller_load_validated="$(jq -r '.response.controller_execution.load_validated // false' "$load_validation_evidence_file")"
    load_validation_controller_isolated_pool="$(jq -r '.response.controller_execution.isolated_worker_pool_configured // false' "$load_validation_evidence_file")"
  fi
  isolated_pool="$(jq -r '.load_validation.isolated_worker_pool_configured // false' "$readiness_file")"
  load_validation_controller_fresh="$(jq -r '.load_validation.controller_evidence_fresh // false' "$readiness_file")"
  load_validation_controller_age_hours="$(jq -r '.load_validation.latest_controller_age_hours // "none"' "$readiness_file")"
  production_ops_status="$(jq -r '.production_ops.status // "unknown"' "$readiness_file")"
  blocked_count="$(jq -r 'if .production_ops.production_blocked == true then 1 else 0 end' "$readiness_file")"
  [[ "$load_validation_evidence_status" == "captured" ]] || blocked_count=$((blocked_count + 1))
  [[ "$load_validation_controller_status" == "validated" ]] || blocked_count=$((blocked_count + 1))
  is_real_cluster_kind "$load_validation_target_kind" || blocked_count=$((blocked_count + 1))
  is_multi_node "$load_validation_node_count" || blocked_count=$((blocked_count + 1))
  is_production_identity "$load_validation_cluster_id" || blocked_count=$((blocked_count + 1))
  [[ "$load_validation_controller_load_validated" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$load_validation_controller_isolated_pool" == "true" ]] || blocked_count=$((blocked_count + 1))
  if [[ -n "$PRODUCTION_CLUSTER_ID" ]]; then
    is_production_identity "$PRODUCTION_CLUSTER_ID" || blocked_count=$((blocked_count + 1))
    [[ "$load_validation_cluster_id" == "$PRODUCTION_CLUSTER_ID" ]] || blocked_count=$((blocked_count + 1))
  fi

  {
    echo "worker_readiness_status=$status"
    echo "readiness_score=$readiness_score"
    echo "queue_backend=$queue_backend"
    echo "queue_durable=$queue_durable"
    echo "worker_mode=$worker_mode"
    echo "k8s_hardening_status=$k8s_hardening"
    echo "autoscaling_status=$autoscaling_status"
    echo "load_validated=$load_validated"
    echo "load_validation_evidence_status=$load_validation_evidence_status"
    echo "load_validation_run_status=$load_validation_run_status"
    echo "load_validation_controller_status=$load_validation_controller_status"
    echo "load_validation_target_kind=$load_validation_target_kind"
    echo "load_validation_node_count=$load_validation_node_count"
    echo "load_validation_cluster_id=$load_validation_cluster_id"
    echo "load_validation_cluster_profile=$load_validation_cluster_profile"
    echo "load_validation_controller_load_validated=$load_validation_controller_load_validated"
    echo "load_validation_controller_isolated_worker_pool_configured=$load_validation_controller_isolated_pool"
    echo "expected_production_cluster_id=${PRODUCTION_CLUSTER_ID:-<unset>}"
    echo "isolated_worker_pool_configured=$isolated_pool"
    echo "load_validation_controller_evidence_fresh=$load_validation_controller_fresh"
    echo "load_validation_controller_age_hours=$load_validation_controller_age_hours"
    echo "production_ops_status=$production_ops_status"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "production_ops_blocking_reasons:"
    jq -r '.production_ops.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    if [[ "$load_validation_evidence_status" != "captured" ]]; then
      echo "- worker load-validation evidence was not captured: $load_validation_evidence_status"
    fi
    if [[ "$load_validation_controller_status" != "validated" ]]; then
      echo "- worker load-validation controller did not validate the real cluster target: $load_validation_controller_status"
    fi
    if ! is_real_cluster_kind "$load_validation_target_kind"; then
      echo "- worker load-validation target is not a real cluster kind: $load_validation_target_kind"
    fi
    if ! is_multi_node "$load_validation_node_count"; then
      echo "- worker load-validation did not report a multi-node cluster: node_count=$load_validation_node_count"
    fi
    if ! is_production_identity "$load_validation_cluster_id"; then
      echo "- worker load-validation cluster id is pilot/mock/local: ${load_validation_cluster_id:-<empty>}"
    fi
    if [[ "$load_validation_controller_load_validated" != "true" ]]; then
      echo "- worker load-validation controller did not report load_validated=true"
    fi
    if [[ "$load_validation_controller_isolated_pool" != "true" ]]; then
      echo "- worker load-validation controller did not report isolated_worker_pool_configured=true"
    fi
    if [[ -n "$PRODUCTION_CLUSTER_ID" ]]; then
      if ! is_production_identity "$PRODUCTION_CLUSTER_ID"; then
        echo "- configured MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID is pilot/mock/local: $PRODUCTION_CLUSTER_ID"
      fi
      if [[ "$load_validation_cluster_id" != "$PRODUCTION_CLUSTER_ID" ]]; then
        echo "- worker load-validation cluster id does not match MANDOFORGE_STAGE2_PRODUCTION_CLUSTER_ID"
      fi
    fi
    echo
    echo "runbook_actions:"
    jq -r '.runbook_actions[]? | "- \(.)"' "$readiness_file"
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Worker evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_load_validation_evidence() {
  local load_validation_response
  local load_validation_evidence_file="$EVIDENCE_DIR/worker-load-validation-evidence.json"

  load_validation_response="$(fetch_json POST /api/execution-jobs/worker-load-validation/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$load_validation_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$load_validation_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$load_validation_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/execution-jobs/worker-readiness >/dev/null
capture_load_validation_evidence
fetch_json GET /api/execution-jobs/worker-readiness >/dev/null
write_summary
