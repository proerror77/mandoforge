#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8787}"
SUBJECT="${MANDOFORGE_STAGE2_GATE_SUBJECT:-remote-computer-evidence-gate}"
ROLES="${MANDOFORGE_STAGE2_GATE_ROLES:-admin}"
EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/remote-computer-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_SIDECAR_RECOVERY="${RUN_STAGE2_REMOTE_SIDECAR_RECOVERY:-0}"
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
    echo "remote computer evidence gate requires $1" >&2
    exit 1
  fi
}

is_multi_node() {
  local value="$1"
  [[ "$value" =~ ^[0-9]+$ && "$value" -ge 2 ]]
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

is_distributed_state_backend() {
  local value="$1"
  case "$value" in
    juicefs|cephfs|longhorn-rwx)
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
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
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
    echo "remote computer evidence request failed: $method $path returned HTTP $http_status" >&2
    sed -n '1,40p' "$response_body" >&2
    if [[ "$http_status" == "403" ]] && grep -q "x-mandoforge-roles is only accepted in explicit insecure dev auth mode" "$response_body"; then
      echo "hint: start the local API with MANDOFORGE_INSECURE_DEV_AUTH=1 when using this dev-header evidence gate." >&2
    fi
    rm -f "$response_body"
    exit 1
  fi

  tee "$target" <"$response_body" >/dev/null
  rm -f "$response_body"
  echo "$target"
}

write_summary() {
  local readiness_file="$EVIDENCE_DIR/api-remote-computers-readiness.json"
  local runner_file="$EVIDENCE_DIR/api-remote-computers-runner-readiness.json"
  local state_sync_evidence_file="$EVIDENCE_DIR/remote-computer-state-sync-evidence.json"
  local sidecar_recovery_evidence_file="$EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"
  local summary_file="$EVIDENCE_DIR/summary.txt"
  local status
  local score
  local state_sync_status
  local sidecar_supervision_status
  local sidecar_recovery_status
  local runner_status
  local runner_configured
  local runner_ready
  local runner_message
  local state_sync_evidence_status
  local state_sync_validation_status
  local state_sync_controller_status
  local state_sync_controller_fresh
  local state_sync_controller_age_hours
  local state_sync_target_kind
  local state_sync_node_count
  local state_sync_cluster_id
  local state_sync_backend
  local state_sync_state_claim
  local state_sync_checked_path_count
  local state_sync_checked_path_detail_count
  local sidecar_recovery_evidence_status
  local sidecar_recovery_run_status
  local sidecar_validation_status
  local sidecar_target_kind
  local sidecar_node_count
  local sidecar_cluster_id
  local sidecar_replacement_scope
  local sidecar_replacement_pods_healthy
  local sidecar_checked_pod_count
  local sidecar_checked_pod_detail_count
  local blocked_count

  status="$(jq -r '.status // "unknown"' "$readiness_file")"
  score="$(jq -r '.readiness_score // 0' "$readiness_file")"
  state_sync_status="$(jq -r '.production_state_sync.status // "unknown"' "$readiness_file")"
  state_sync_controller_fresh="$(jq -r '.production_state_sync.controller_evidence_fresh // false' "$readiness_file")"
  state_sync_controller_age_hours="$(jq -r '.production_state_sync.latest_controller_age_hours // "none"' "$readiness_file")"
  sidecar_supervision_status="$(jq -r '.sidecar_supervision.status // "unknown"' "$readiness_file")"
  sidecar_recovery_status="$(jq -r '.sidecar_recovery.status // "unknown"' "$readiness_file")"
  runner_status="$(jq -r '.status // "unknown"' "$runner_file")"
  runner_configured="$(jq -r '.configured // false' "$runner_file")"
  runner_ready="$(jq -r '(.configured == true) and (((.status // "") == "ready") or ((.status // "") == "dry_run_ready") or ((.status // "") == "live_ready"))' "$runner_file")"
  runner_message="$(jq -r '.message // ""' "$runner_file")"
  state_sync_evidence_status="missing"
  state_sync_validation_status="unknown"
  if [[ -s "$state_sync_evidence_file" ]]; then
    state_sync_evidence_status="$(jq -r '.status // "unknown"' "$state_sync_evidence_file")"
    state_sync_validation_status="$(jq -r '.response.status // "unknown"' "$state_sync_evidence_file")"
  fi
  state_sync_controller_status="unknown"
  state_sync_target_kind="unknown"
  state_sync_node_count="0"
  state_sync_cluster_id=""
  state_sync_backend="unknown"
  state_sync_state_claim=""
  state_sync_checked_path_count="0"
  state_sync_checked_path_detail_count="0"
  if [[ -s "$state_sync_evidence_file" ]]; then
    state_sync_controller_status="$(jq -r '.response.controller_execution.status // "unknown"' "$state_sync_evidence_file")"
    state_sync_target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$state_sync_evidence_file")"
    state_sync_node_count="$(jq -r '.response.controller_execution.node_count // 0' "$state_sync_evidence_file")"
    state_sync_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$state_sync_evidence_file")"
    state_sync_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // "unknown"' "$state_sync_evidence_file")"
    state_sync_state_claim="$(jq -r '.response.controller_execution.state_claim // ""' "$state_sync_evidence_file")"
    state_sync_checked_path_count="$(jq -r '.response.controller_execution.checked_path_count // 0' "$state_sync_evidence_file")"
    state_sync_checked_path_detail_count="$(jq -r '(.response.controller_execution.state_claim // "") as $state_claim | (.response.controller_execution.cluster_id // "") as $cluster_id | [
      (
        .response.controller_execution.checked_paths[]?,
        .response.controller_execution.checked_state_paths[]?,
        .response.controller_execution.path_checks[]?
      )
      | select(
          type == "object"
          and ($state_claim | length > 0)
          and (($cluster_id // "") | length > 0)
          and ((.path // .state_path // .name // "") | length > 0)
          and ((.state_claim // .claim // .pvc // .persistent_volume_claim // "") == $state_claim)
          and ((.cluster_id // .state_sync_cluster_id // .target_cluster_id // "") == $cluster_id)
          and ((.status // .result // .health // "") | ascii_downcase | IN("passed", "validated", "completed", "ready", "exists", "mounted", "available", "ok", "healthy", "accessible", "readable", "writable"))
          and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
        )
    ] | length' "$state_sync_evidence_file")"
  fi
  sidecar_recovery_evidence_status="not_requested"
  sidecar_recovery_run_status="not_run"
  sidecar_validation_status="not_requested"
  sidecar_target_kind="unknown"
  sidecar_node_count="0"
  sidecar_cluster_id=""
  sidecar_replacement_scope="unknown"
  sidecar_replacement_pods_healthy="false"
  sidecar_checked_pod_count="0"
  sidecar_checked_pod_detail_count="0"
  if [[ -s "$sidecar_recovery_evidence_file" ]]; then
    sidecar_recovery_evidence_status="$(jq -r '.status // "unknown"' "$sidecar_recovery_evidence_file")"
    sidecar_recovery_run_status="$(jq -r '.response.status // "unknown"' "$sidecar_recovery_evidence_file")"
    sidecar_validation_status="$(jq -r '.response.validation_result.status // "unknown"' "$sidecar_recovery_evidence_file")"
    sidecar_target_kind="$(jq -r '.response.validation_result.target_kind // "unknown"' "$sidecar_recovery_evidence_file")"
    sidecar_node_count="$(jq -r '.response.validation_result.node_count // 0' "$sidecar_recovery_evidence_file")"
    sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$sidecar_recovery_evidence_file")"
    sidecar_replacement_scope="$(jq -r '.response.validation_result.replacement_scope // "unknown"' "$sidecar_recovery_evidence_file")"
    sidecar_replacement_pods_healthy="$(jq -r '.response.validation_result.replacement_pods_healthy // false' "$sidecar_recovery_evidence_file")"
    sidecar_checked_pod_count="$(jq -r '.response.validation_result.checked_pod_count // 0' "$sidecar_recovery_evidence_file")"
    sidecar_checked_pod_detail_count="$(jq -r '(.response.validation_result.cluster_id // "") as $cluster_id | [
      (
        .response.validation_result.checked_pods[]?,
        .response.validation_result.replacement_pods[]?,
        .response.validation_result.pod_checks[]?
      )
      | select(
          type == "object"
          and ((.pod // .pod_name // .name // "") | length > 0)
          and (($cluster_id // "") | length > 0)
          and ((.cluster_id // .sidecar_cluster_id // .target_cluster_id // "") == $cluster_id)
          and ((.status // .phase // .health // "") | ascii_downcase | IN("running", "ready", "healthy", "succeeded", "validated"))
          and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .validated_at // .timestamp // "") | length > 0)
        )
    ] | length' "$sidecar_recovery_evidence_file")"
  fi
  blocked_count="$(jq -r '[
      .production_state_sync.production_blocked,
      (.sidecar_recovery.status == "blocked")
    ] | map(select(. == true)) | length' "$readiness_file")"
  if [[ "$runner_ready" != "true" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$state_sync_controller_status" != "validated" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$state_sync_validation_status" != "validated" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_real_cluster_kind "$state_sync_target_kind"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_multi_node "$state_sync_node_count"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_production_identity "$state_sync_cluster_id"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if ! is_distributed_state_backend "$state_sync_backend"; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ -z "$state_sync_state_claim" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$state_sync_checked_path_count" =~ ^[0-9]+$ || "$state_sync_checked_path_count" == "0" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ ! "$state_sync_checked_path_detail_count" =~ ^[0-9]+$ || ! "$state_sync_checked_path_count" =~ ^[0-9]+$ || "$state_sync_checked_path_detail_count" -lt "$state_sync_checked_path_count" ]]; then
    blocked_count="$((blocked_count + 1))"
  fi
  if [[ "$RUN_SIDECAR_RECOVERY" == "1" ]]; then
    if [[ "$sidecar_validation_status" != "validated" ]]; then
      blocked_count="$((blocked_count + 1))"
    fi
    if ! is_real_cluster_kind "$sidecar_target_kind"; then
      blocked_count="$((blocked_count + 1))"
    fi
    if ! is_multi_node "$sidecar_node_count"; then
      blocked_count="$((blocked_count + 1))"
    fi
    if ! is_production_identity "$sidecar_cluster_id"; then
      blocked_count="$((blocked_count + 1))"
    fi
    if [[ "$sidecar_replacement_scope" != "cluster" ]]; then
      blocked_count="$((blocked_count + 1))"
    fi
    if [[ "$sidecar_replacement_pods_healthy" != "true" ]]; then
      blocked_count="$((blocked_count + 1))"
    fi
    if [[ ! "$sidecar_checked_pod_count" =~ ^[0-9]+$ || "$sidecar_checked_pod_count" == "0" ]]; then
      blocked_count="$((blocked_count + 1))"
    fi
    if [[ ! "$sidecar_checked_pod_detail_count" =~ ^[0-9]+$ || ! "$sidecar_checked_pod_count" =~ ^[0-9]+$ || "$sidecar_checked_pod_detail_count" -lt "$sidecar_checked_pod_count" ]]; then
      blocked_count="$((blocked_count + 1))"
    fi
  fi

  {
    echo "remote_computer_status=$status"
    echo "readiness_score=$score"
    echo "production_state_sync_status=$state_sync_status"
    echo "sidecar_supervision_status=$sidecar_supervision_status"
    echo "sidecar_recovery_status=$sidecar_recovery_status"
    echo "runner_status=$runner_status"
    echo "runner_configured=$runner_configured"
    echo "runner_ready=$runner_ready"
    echo "state_sync_evidence_status=$state_sync_evidence_status"
    echo "state_sync_validation_status=$state_sync_validation_status"
    echo "state_sync_controller_status=$state_sync_controller_status"
    echo "state_sync_controller_evidence_fresh=$state_sync_controller_fresh"
    echo "state_sync_controller_age_hours=$state_sync_controller_age_hours"
    echo "state_sync_target_kind=$state_sync_target_kind"
    echo "state_sync_node_count=$state_sync_node_count"
    echo "state_sync_cluster_id=$state_sync_cluster_id"
    echo "state_sync_backend=$state_sync_backend"
    echo "state_sync_state_claim=$state_sync_state_claim"
    echo "state_sync_checked_path_count=$state_sync_checked_path_count"
    echo "state_sync_checked_path_detail_count=$state_sync_checked_path_detail_count"
    echo "sidecar_recovery_evidence_status=$sidecar_recovery_evidence_status"
    echo "sidecar_recovery_run_status=$sidecar_recovery_run_status"
    echo "sidecar_validation_status=$sidecar_validation_status"
    echo "sidecar_target_kind=$sidecar_target_kind"
    echo "sidecar_node_count=$sidecar_node_count"
    echo "sidecar_cluster_id=$sidecar_cluster_id"
    echo "sidecar_replacement_scope=$sidecar_replacement_scope"
    echo "sidecar_replacement_pods_healthy=$sidecar_replacement_pods_healthy"
    echo "sidecar_checked_pod_count=$sidecar_checked_pod_count"
    echo "sidecar_checked_pod_detail_count=$sidecar_checked_pod_detail_count"
    echo "production_blocked_count=$blocked_count"
    echo "evidence_dir=$EVIDENCE_DIR"
    echo "sidecar_recovery_run=$RUN_SIDECAR_RECOVERY"
    echo
    echo "attention_items:"
    jq -r '.attention_items[]? | "- \(.severity): \(.kind) - \(.message)"' "$readiness_file"
    echo
    echo "state_sync_blocking_reasons:"
    jq -r '.production_state_sync.blocking_reasons[]? | "- \(.)"' "$readiness_file"
    if [[ "$runner_ready" != "true" ]]; then
      echo "- remote computer runner is not ready: status=$runner_status configured=$runner_configured message=$runner_message"
    fi
    if [[ "$state_sync_controller_status" != "validated" ]]; then
      echo "- state-sync controller did not validate the distributed state contract: $state_sync_controller_status"
    fi
    if [[ "$state_sync_validation_status" != "validated" ]]; then
      echo "- state-sync validation response is not validated: $state_sync_validation_status"
    fi
    if ! is_real_cluster_kind "$state_sync_target_kind"; then
      echo "- state-sync target is not a real cluster kind: $state_sync_target_kind"
    fi
    if ! is_multi_node "$state_sync_node_count"; then
      echo "- state-sync did not report a multi-node cluster: node_count=$state_sync_node_count"
    fi
    if ! is_production_identity "$state_sync_cluster_id"; then
      echo "- state-sync cluster id is pilot/mock/local: ${state_sync_cluster_id:-<empty>}"
    fi
    if ! is_distributed_state_backend "$state_sync_backend"; then
      echo "- state-sync backend is not a supported distributed filesystem: $state_sync_backend"
    fi
    if [[ -z "$state_sync_state_claim" ]]; then
      echo "- state-sync controller did not report a state claim"
    fi
    if [[ ! "$state_sync_checked_path_count" =~ ^[0-9]+$ || "$state_sync_checked_path_count" == "0" ]]; then
      echo "- state-sync controller did not report any checked state contract paths: checked_path_count=$state_sync_checked_path_count"
    fi
    if [[ ! "$state_sync_checked_path_detail_count" =~ ^[0-9]+$ || ! "$state_sync_checked_path_count" =~ ^[0-9]+$ || "$state_sync_checked_path_detail_count" -lt "$state_sync_checked_path_count" ]]; then
      echo "- state-sync controller did not include checked path details bound to the same cluster and state claim for every counted path: checked_path_detail_count=$state_sync_checked_path_detail_count checked_path_count=$state_sync_checked_path_count"
    fi
    if [[ "$RUN_SIDECAR_RECOVERY" == "1" ]]; then
      if [[ "$sidecar_validation_status" != "validated" ]]; then
        echo "- sidecar recovery validation controller did not validate replacement: $sidecar_validation_status"
      fi
      if ! is_real_cluster_kind "$sidecar_target_kind"; then
        echo "- sidecar recovery target is not a real cluster kind: $sidecar_target_kind"
      fi
      if ! is_multi_node "$sidecar_node_count"; then
        echo "- sidecar recovery did not report a multi-node cluster: node_count=$sidecar_node_count"
      fi
      if ! is_production_identity "$sidecar_cluster_id"; then
        echo "- sidecar recovery cluster id is pilot/mock/local: ${sidecar_cluster_id:-<empty>}"
      fi
      if [[ "$sidecar_replacement_scope" != "cluster" ]]; then
        echo "- sidecar recovery replacement scope is not cluster-wide: $sidecar_replacement_scope"
      fi
      if [[ "$sidecar_replacement_pods_healthy" != "true" ]]; then
        echo "- sidecar recovery validation did not report healthy replacement Pods"
      fi
      if [[ ! "$sidecar_checked_pod_count" =~ ^[0-9]+$ || "$sidecar_checked_pod_count" == "0" ]]; then
        echo "- sidecar recovery validation did not report any checked Pods: checked_pod_count=$sidecar_checked_pod_count"
      fi
      if [[ ! "$sidecar_checked_pod_detail_count" =~ ^[0-9]+$ || ! "$sidecar_checked_pod_count" =~ ^[0-9]+$ || "$sidecar_checked_pod_detail_count" -lt "$sidecar_checked_pod_count" ]]; then
        echo "- sidecar recovery validation did not include checked Pod details bound to the same cluster for every counted Pod: checked_pod_detail_count=$sidecar_checked_pod_detail_count checked_pod_count=$sidecar_checked_pod_count"
      fi
    fi
    echo
    echo "runbook_actions:"
    jq -r '.runbook_actions[]? | "- \(.)"' "$readiness_file"
    if [[ "$runner_ready" != "true" ]]; then
      echo "- configure the Remote Computer runner before declaring Remote Computer evidence ready"
    fi
  } >"$summary_file"

  cat "$summary_file"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Remote Computer evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

capture_state_sync_evidence() {
  local state_sync_response
  local state_sync_evidence_file="$EVIDENCE_DIR/remote-computer-state-sync-evidence.json"

  state_sync_response="$(fetch_json POST /api/remote-computers/state-sync/validate)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$state_sync_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$state_sync_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$state_sync_evidence_file"
}

capture_sidecar_recovery_evidence() {
  local sidecar_recovery_response
  local sidecar_recovery_evidence_file="$EVIDENCE_DIR/remote-computer-sidecar-recovery-evidence.json"

  sidecar_recovery_response="$(fetch_json POST /api/remote-computers/sidecars/recovery/run)"
  jq -n \
    --arg status "captured" \
    --arg response_file "$sidecar_recovery_response" \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --slurpfile response "$sidecar_recovery_response" \
    '{
      status: $status,
      generated_at: $generated_at,
      response_file: $response_file,
      response: ($response[0] // {})
    }' >"$sidecar_recovery_evidence_file"
}

require_cmd curl
require_cmd jq
mkdir -p "$EVIDENCE_DIR"

curl -fsS "$BASE_URL/healthz" >/dev/null
fetch_json GET /api/remote-computers/readiness >/dev/null
fetch_json GET /api/remote-computers/runner/readiness >/dev/null
capture_state_sync_evidence

if [[ "$RUN_SIDECAR_RECOVERY" == "1" ]]; then
  capture_sidecar_recovery_evidence
else
  echo "skipping Remote Computer sidecar recovery; set RUN_STAGE2_REMOTE_SIDECAR_RECOVERY=1 to include replacement evidence" >&2
fi

fetch_json GET /api/remote-computers/readiness >/dev/null
fetch_json GET /api/remote-computers/runner/readiness >/dev/null
write_summary
