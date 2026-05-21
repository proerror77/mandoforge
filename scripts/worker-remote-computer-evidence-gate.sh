#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/worker-remote-computer-evidence}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"
RUN_STAGE2_REMOTE_SIDECAR_RECOVERY="${RUN_STAGE2_REMOTE_SIDECAR_RECOVERY:-1}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "worker/remote-computer evidence gate requires $1" >&2
    exit 1
  fi
}

run_gate() {
  local gate_name="$1"
  local script_path="$2"
  local output_dir="$3"
  local stdout_file="$4"
  local stderr_file="$5"

  mkdir -p "$output_dir"
  set +e
  EVIDENCE_DIR="$output_dir" \
    ALLOW_BLOCKED=1 \
    RUN_STAGE2_REMOTE_SIDECAR_RECOVERY="$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" \
    "$script_path" >"$stdout_file" 2>"$stderr_file"
  local exit_code="$?"
  set -e

  jq -n \
    --arg gate "$gate_name" \
    --arg script "$script_path" \
    --arg output_dir "$output_dir" \
    --arg stdout_file "$stdout_file" \
    --arg stderr_file "$stderr_file" \
    --argjson exit_code "$exit_code" \
    '{
      gate: $gate,
      script: $script,
      output_dir: $output_dir,
      stdout_file: $stdout_file,
      stderr_file: $stderr_file,
      exit_code: $exit_code,
      completed: ($exit_code == 0)
    }' >"$EVIDENCE_DIR/$gate_name-run.json"

  if [[ "$exit_code" != "0" ]]; then
    echo "$gate_name evidence gate failed before readiness evaluation" >&2
    sed -n '1,80p' "$stderr_file" >&2
    exit "$exit_code"
  fi
}

bool_json() {
  local value="$1"
  if [[ "$value" == "true" ]]; then
    printf 'true'
  else
    printf 'false'
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

worker_load_check_detail_count() {
  jq -r '[
    (
      .response.controller_execution.load_checks[]?,
      .response.controller_execution.worker_pool_checks[]?,
      .response.controller_execution.validation_checks[]?,
      .response.controller_execution.load_validation_checks[]?,
      .response.controller_execution.checks[]?
    )
    | select(
        type == "object"
        and ((.name // .check // .kind // "") | length > 0)
        and ((.worker_pool // .pool_id // .queue // .queue_name // "") | length > 0)
        and ((.status // .result // "") | ascii_downcase | IN("passed", "validated", "completed"))
        and ((.audit_id // .audit_log_id // .trace_id // .run_id // .checked_at // .executed_at // .validated_at // .timestamp // "") | length > 0)
      )
  ] | length' "$1" 2>/dev/null || echo "0"
}

write_summary() {
  local worker_dir="$EVIDENCE_DIR/worker"
  local remote_dir="$EVIDENCE_DIR/remote-computer"
  local worker_readiness="$worker_dir/api-execution-jobs-worker-readiness.json"
  local worker_validation="$worker_dir/worker-load-validation-evidence.json"
  local remote_readiness="$remote_dir/api-remote-computers-readiness.json"
  local runner_readiness="$remote_dir/api-remote-computers-runner-readiness.json"
  local state_sync="$remote_dir/remote-computer-state-sync-evidence.json"
  local sidecar_recovery="$remote_dir/remote-computer-sidecar-recovery-evidence.json"
  local summary_json="$EVIDENCE_DIR/summary.json"
  local summary_txt="$EVIDENCE_DIR/summary.txt"

  for required_file in \
    "$worker_readiness" \
    "$worker_validation" \
    "$remote_readiness" \
    "$runner_readiness" \
    "$state_sync"
  do
    if [[ ! -s "$required_file" ]]; then
      echo "missing required evidence artifact: $required_file" >&2
      exit 1
    fi
  done

  if [[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" == "1" && ! -s "$sidecar_recovery" ]]; then
    echo "missing required sidecar recovery evidence: $sidecar_recovery" >&2
    exit 1
  fi

  local worker_production_ready
  local isolated_pool
  local load_validated
  local worker_controller_fresh
  local worker_validation_status
  local worker_validation_response_status
  local state_sync_evidence_status
  local remote_state_ready
  local state_sync_response_status
  local state_controller_fresh
  local sidecar_recovery_ready
  local sidecar_recovery_evidence_status
  local sidecar_recovery_response_status
  local sidecar_validation_status
  local runner_ready
  local worker_cluster_id
  local worker_target_kind
  local worker_node_count
  local worker_cluster_profile
  local worker_load_check_detail_count
  local state_cluster_id
  local state_target_kind
  local state_node_count
  local state_backend
  local state_claim
  local state_checked_path_count
  local state_checked_path_detail_count
  local state_checked_paths_json
  local state_cluster_profile
  local sidecar_cluster_id
  local sidecar_target_kind
  local sidecar_node_count
  local sidecar_replacement_scope
  local sidecar_replacement_pods_healthy
  local sidecar_checked_pod_count
  local sidecar_checked_pod_detail_count
  local sidecar_checked_pods_json
  local same_cluster_target

  worker_production_ready="$(jq -r '.production_ops.status == "ready" and (.production_ops.production_blocked == false)' "$worker_readiness")"
  isolated_pool="$(jq -r '.load_validation.isolated_worker_pool_configured == true' "$worker_readiness")"
  load_validated="$(jq -r '.load_validation.load_validated == true' "$worker_readiness")"
  worker_controller_fresh="$(jq -r '.load_validation.controller_evidence_fresh == true' "$worker_readiness")"
  worker_validation_status="$(jq -r '.status // "unknown"' "$worker_validation")"
  worker_validation_response_status="$(jq -r '.response.status // "unknown"' "$worker_validation")"
  worker_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$worker_validation")"
  worker_target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$worker_validation")"
  worker_node_count="$(jq -r '.response.controller_execution.node_count // 0' "$worker_validation")"
  worker_cluster_profile="$(jq -r '.response.controller_execution.cluster_profile // "unknown"' "$worker_validation")"
  worker_load_check_detail_count="$(worker_load_check_detail_count "$worker_validation")"

  remote_state_ready="$(jq -r '.production_state_sync.status == "ready" and (.production_state_sync.production_blocked == false)' "$remote_readiness")"
  state_sync_evidence_status="$(jq -r '.status // "unknown"' "$state_sync")"
  state_sync_response_status="$(jq -r '.response.status // "unknown"' "$state_sync")"
  state_controller_fresh="$(jq -r '.production_state_sync.controller_evidence_fresh == true' "$remote_readiness")"
  state_cluster_id="$(jq -r '.response.controller_execution.cluster_id // ""' "$state_sync")"
  state_target_kind="$(jq -r '.response.controller_execution.target_kind // "unknown"' "$state_sync")"
  state_node_count="$(jq -r '.response.controller_execution.node_count // 0' "$state_sync")"
  state_backend="$(jq -r '.response.controller_execution.distributed_state_backend // .response.controller_execution.storage_backend // .response.controller_execution.state_backend // .response.controller_execution.provider // "unknown"' "$state_sync")"
  state_claim="$(jq -r '.response.controller_execution.state_claim // ""' "$state_sync")"
  state_checked_path_count="$(jq -r '.response.controller_execution.checked_path_count // 0' "$state_sync")"
  state_checked_paths_json="$(jq -c '[
    (
      .response.controller_execution.checked_paths[]?,
      .response.controller_execution.checked_state_paths[]?,
      .response.controller_execution.path_checks[]?
    )
    | select(
        type == "object"
        and ((.path // .state_path // .name // "") | length > 0)
        and ((.status // .result // .health // "") | ascii_downcase | IN("passed", "validated", "completed", "ready", "exists", "mounted", "available", "ok", "healthy", "accessible", "readable", "writable"))
      )
  ]' "$state_sync")"
  state_checked_path_detail_count="$(jq -r 'length' <<<"$state_checked_paths_json")"
  state_cluster_profile="$(jq -r '.response.controller_execution.cluster_profile // "unknown"' "$state_sync")"
  sidecar_recovery_ready="$(jq -r '.sidecar_recovery.status == "ready"' "$remote_readiness")"
  sidecar_recovery_evidence_status="not_requested"
  sidecar_recovery_response_status="not_requested"
  sidecar_validation_status="not_requested"
  sidecar_cluster_id=""
  sidecar_target_kind="unknown"
  sidecar_node_count="0"
  sidecar_replacement_scope="unknown"
  sidecar_replacement_pods_healthy="false"
  sidecar_checked_pod_count="0"
  sidecar_checked_pod_detail_count="0"
  sidecar_checked_pods_json="[]"
  if [[ -s "$sidecar_recovery" ]]; then
    sidecar_recovery_evidence_status="$(jq -r '.status // "unknown"' "$sidecar_recovery")"
    sidecar_recovery_response_status="$(jq -r '.response.status // "unknown"' "$sidecar_recovery")"
    sidecar_validation_status="$(jq -r '.response.validation_result.status // "unknown"' "$sidecar_recovery")"
    sidecar_cluster_id="$(jq -r '.response.validation_result.cluster_id // ""' "$sidecar_recovery")"
    sidecar_target_kind="$(jq -r '.response.validation_result.target_kind // "unknown"' "$sidecar_recovery")"
    sidecar_node_count="$(jq -r '.response.validation_result.node_count // 0' "$sidecar_recovery")"
    sidecar_replacement_scope="$(jq -r '.response.validation_result.replacement_scope // "unknown"' "$sidecar_recovery")"
    sidecar_replacement_pods_healthy="$(jq -r '.response.validation_result.replacement_pods_healthy // false' "$sidecar_recovery")"
    sidecar_checked_pod_count="$(jq -r '.response.validation_result.checked_pod_count // 0' "$sidecar_recovery")"
    sidecar_checked_pods_json="$(jq -c '[
      (
        .response.validation_result.checked_pods[]?,
        .response.validation_result.replacement_pods[]?,
        .response.validation_result.pod_checks[]?
      )
      | select(
          type == "object"
          and ((.pod // .pod_name // .name // "") | length > 0)
          and ((.status // .phase // .health // "") | ascii_downcase | IN("running", "ready", "healthy", "succeeded", "validated"))
        )
    ]' "$sidecar_recovery")"
    sidecar_checked_pod_detail_count="$(jq -r 'length' <<<"$sidecar_checked_pods_json")"
  fi
  runner_ready="$(jq -r '(.configured == true) and (((.status // "") == "ready") or ((.status // "") == "dry_run_ready") or ((.status // "") == "live_ready"))' "$runner_readiness")"
  same_cluster_target="false"
  if [[ -n "$worker_cluster_id" && "$worker_cluster_id" == "$state_cluster_id" ]]; then
    if [[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" != "1" || "$worker_cluster_id" == "$sidecar_cluster_id" ]]; then
      same_cluster_target="true"
    fi
  fi

  local blocked_count=0
  [[ "$worker_production_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$isolated_pool" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$load_validated" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$worker_controller_fresh" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$worker_validation_status" == "captured" ]] || blocked_count=$((blocked_count + 1))
  is_real_cluster_kind "$worker_target_kind" || blocked_count=$((blocked_count + 1))
  is_multi_node "$worker_node_count" || blocked_count=$((blocked_count + 1))
  is_production_identity "$worker_cluster_id" || blocked_count=$((blocked_count + 1))
  [[ "$worker_load_check_detail_count" =~ ^[0-9]+$ && "$worker_load_check_detail_count" -gt 0 ]] || blocked_count=$((blocked_count + 1))
  [[ "$remote_state_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$state_sync_evidence_status" == "captured" ]] || blocked_count=$((blocked_count + 1))
  [[ "$state_controller_fresh" == "true" ]] || blocked_count=$((blocked_count + 1))
  is_real_cluster_kind "$state_target_kind" || blocked_count=$((blocked_count + 1))
  is_multi_node "$state_node_count" || blocked_count=$((blocked_count + 1))
  is_production_identity "$state_cluster_id" || blocked_count=$((blocked_count + 1))
  is_distributed_state_backend "$state_backend" || blocked_count=$((blocked_count + 1))
  [[ -n "$state_claim" ]] || blocked_count=$((blocked_count + 1))
  [[ "$state_checked_path_count" =~ ^[0-9]+$ && "$state_checked_path_count" -gt 0 ]] || blocked_count=$((blocked_count + 1))
  [[ "$state_checked_path_detail_count" =~ ^[0-9]+$ && "$state_checked_path_count" =~ ^[0-9]+$ && "$state_checked_path_detail_count" -ge "$state_checked_path_count" ]] || blocked_count=$((blocked_count + 1))
  [[ "$same_cluster_target" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$runner_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
  if [[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" == "1" ]]; then
    [[ "$sidecar_recovery_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_recovery_evidence_status" == "captured" ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_recovery_response_status" == "validated" || "$sidecar_recovery_response_status" == "recovered" || "$sidecar_recovery_response_status" == "ready" || "$sidecar_recovery_response_status" == "completed" ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_validation_status" == "validated" ]] || blocked_count=$((blocked_count + 1))
    is_real_cluster_kind "$sidecar_target_kind" || blocked_count=$((blocked_count + 1))
    is_multi_node "$sidecar_node_count" || blocked_count=$((blocked_count + 1))
    is_production_identity "$sidecar_cluster_id" || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_replacement_scope" == "cluster" ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_replacement_pods_healthy" == "true" ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_checked_pod_count" =~ ^[0-9]+$ && "$sidecar_checked_pod_count" -gt 0 ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_checked_pod_detail_count" =~ ^[0-9]+$ && "$sidecar_checked_pod_count" =~ ^[0-9]+$ && "$sidecar_checked_pod_detail_count" -ge "$sidecar_checked_pod_count" ]] || blocked_count=$((blocked_count + 1))
  fi

  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg evidence_dir "$EVIDENCE_DIR" \
    --arg worker_readiness "$worker_readiness" \
    --arg worker_validation "$worker_validation" \
    --arg remote_readiness "$remote_readiness" \
    --arg runner_readiness "$runner_readiness" \
    --arg state_sync "$state_sync" \
    --arg sidecar_recovery "$sidecar_recovery" \
    --arg worker_validation_status "$worker_validation_status" \
    --arg worker_validation_response_status "$worker_validation_response_status" \
    --arg worker_cluster_id "$worker_cluster_id" \
    --arg worker_target_kind "$worker_target_kind" \
    --arg worker_node_count "$worker_node_count" \
    --arg worker_cluster_profile "$worker_cluster_profile" \
    --arg worker_load_check_detail_count "$worker_load_check_detail_count" \
    --arg state_sync_response_status "$state_sync_response_status" \
    --arg state_sync_evidence_status "$state_sync_evidence_status" \
    --arg state_cluster_id "$state_cluster_id" \
    --arg state_target_kind "$state_target_kind" \
    --arg state_node_count "$state_node_count" \
    --arg state_backend "$state_backend" \
    --arg state_claim "$state_claim" \
    --arg state_checked_path_count "$state_checked_path_count" \
    --arg state_cluster_profile "$state_cluster_profile" \
    --argjson state_checked_paths "$state_checked_paths_json" \
    --arg sidecar_recovery_response_status "$sidecar_recovery_response_status" \
    --arg sidecar_recovery_evidence_status "$sidecar_recovery_evidence_status" \
    --arg sidecar_validation_status "$sidecar_validation_status" \
    --arg sidecar_cluster_id "$sidecar_cluster_id" \
    --arg sidecar_target_kind "$sidecar_target_kind" \
    --arg sidecar_node_count "$sidecar_node_count" \
    --arg sidecar_replacement_scope "$sidecar_replacement_scope" \
    --arg sidecar_replacement_pods_healthy "$sidecar_replacement_pods_healthy" \
    --arg sidecar_checked_pod_count "$sidecar_checked_pod_count" \
    --argjson sidecar_checked_pods "$sidecar_checked_pods_json" \
    --argjson worker_production_ready "$(bool_json "$worker_production_ready")" \
    --argjson isolated_worker_pool_configured "$(bool_json "$isolated_pool")" \
    --argjson load_validated "$(bool_json "$load_validated")" \
    --argjson worker_controller_evidence_fresh "$(bool_json "$worker_controller_fresh")" \
    --argjson remote_state_sync_ready "$(bool_json "$remote_state_ready")" \
    --argjson remote_state_controller_evidence_fresh "$(bool_json "$state_controller_fresh")" \
    --argjson remote_runner_ready "$(bool_json "$runner_ready")" \
    --argjson sidecar_recovery_required "$(bool_json "$([[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" == "1" ]] && echo true || echo false)")" \
    --argjson sidecar_recovery_ready "$(bool_json "$sidecar_recovery_ready")" \
    --argjson same_cluster_target "$(bool_json "$same_cluster_target")" \
    --argjson production_blocked "$([[ "$blocked_count" == "0" ]] && echo false || echo true)" \
    --argjson production_blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      evidence_dir: $evidence_dir,
      status: (if $production_blocked then "blocked" else "ready" end),
      production_blocked: $production_blocked,
      production_blocked_count: $production_blocked_count,
      worker: {
        readiness_file: $worker_readiness,
        validation_file: $worker_validation,
        production_ready: $worker_production_ready,
        isolated_worker_pool_configured: $isolated_worker_pool_configured,
        load_validated: $load_validated,
        controller_evidence_fresh: $worker_controller_evidence_fresh,
        validation_evidence_status: $worker_validation_status,
        validation_response_status: $worker_validation_response_status,
        cluster_id: $worker_cluster_id,
        target_kind: $worker_target_kind,
        node_count: ($worker_node_count | tonumber),
        cluster_profile: $worker_cluster_profile,
        load_check_detail_count: ($worker_load_check_detail_count | tonumber)
      },
      remote_computer: {
        readiness_file: $remote_readiness,
        runner_readiness_file: $runner_readiness,
        state_sync_file: $state_sync,
        sidecar_recovery_file: $sidecar_recovery,
        state_sync_ready: $remote_state_sync_ready,
        state_controller_evidence_fresh: $remote_state_controller_evidence_fresh,
        state_sync_evidence_status: $state_sync_evidence_status,
        state_sync_response_status: $state_sync_response_status,
        state_sync_cluster_id: $state_cluster_id,
        state_sync_target_kind: $state_target_kind,
        state_sync_node_count: ($state_node_count | tonumber),
        state_sync_cluster_profile: $state_cluster_profile,
        distributed_state_backend: $state_backend,
        state_claim: $state_claim,
        checked_path_count: ($state_checked_path_count | tonumber),
        checked_paths: $state_checked_paths,
        runner_ready: $remote_runner_ready,
        sidecar_recovery_required: $sidecar_recovery_required,
        sidecar_recovery_ready: $sidecar_recovery_ready,
        sidecar_recovery_evidence_status: $sidecar_recovery_evidence_status,
        sidecar_recovery_response_status: $sidecar_recovery_response_status,
        sidecar_validation_status: $sidecar_validation_status,
        sidecar_cluster_id: $sidecar_cluster_id,
        sidecar_target_kind: $sidecar_target_kind,
        sidecar_node_count: ($sidecar_node_count | tonumber),
        sidecar_replacement_scope: $sidecar_replacement_scope,
        replacement_pods_healthy: ($sidecar_replacement_pods_healthy == "true"),
        checked_pod_count: ($sidecar_checked_pod_count | tonumber),
        checked_pods: $sidecar_checked_pods
      },
      same_cluster_target: $same_cluster_target
    }' >"$summary_json"

  {
    jq -r '"worker_remote_computer_status=\(.status)"' "$summary_json"
    jq -r '"production_blocked=\(.production_blocked)"' "$summary_json"
    jq -r '"production_blocked_count=\(.production_blocked_count)"' "$summary_json"
    jq -r '"worker_production_ready=\(.worker.production_ready)"' "$summary_json"
    jq -r '"isolated_worker_pool_configured=\(.worker.isolated_worker_pool_configured)"' "$summary_json"
    jq -r '"worker_load_validated=\(.worker.load_validated)"' "$summary_json"
    jq -r '"worker_controller_evidence_fresh=\(.worker.controller_evidence_fresh)"' "$summary_json"
    jq -r '"worker_cluster_id=\(.worker.cluster_id)"' "$summary_json"
    jq -r '"worker_target_kind=\(.worker.target_kind)"' "$summary_json"
    jq -r '"worker_node_count=\(.worker.node_count)"' "$summary_json"
    jq -r '"worker_load_check_detail_count=\(.worker.load_check_detail_count)"' "$summary_json"
    jq -r '"remote_state_sync_ready=\(.remote_computer.state_sync_ready)"' "$summary_json"
    jq -r '"remote_state_controller_evidence_fresh=\(.remote_computer.state_controller_evidence_fresh)"' "$summary_json"
    jq -r '"remote_state_sync_evidence_status=\(.remote_computer.state_sync_evidence_status)"' "$summary_json"
    jq -r '"remote_state_cluster_id=\(.remote_computer.state_sync_cluster_id)"' "$summary_json"
    jq -r '"remote_state_target_kind=\(.remote_computer.state_sync_target_kind)"' "$summary_json"
    jq -r '"remote_state_node_count=\(.remote_computer.state_sync_node_count)"' "$summary_json"
    jq -r '"distributed_state_backend=\(.remote_computer.distributed_state_backend)"' "$summary_json"
    jq -r '"remote_state_claim=\(.remote_computer.state_claim)"' "$summary_json"
    jq -r '"remote_state_checked_path_count=\(.remote_computer.checked_path_count)"' "$summary_json"
    jq -r '"remote_state_checked_path_detail_count=\(.remote_computer.checked_paths | length)"' "$summary_json"
    jq -r '"remote_runner_ready=\(.remote_computer.runner_ready)"' "$summary_json"
    jq -r '"sidecar_recovery_required=\(.remote_computer.sidecar_recovery_required)"' "$summary_json"
    jq -r '"sidecar_recovery_ready=\(.remote_computer.sidecar_recovery_ready)"' "$summary_json"
    jq -r '"sidecar_recovery_evidence_status=\(.remote_computer.sidecar_recovery_evidence_status)"' "$summary_json"
    jq -r '"sidecar_recovery_response_status=\(.remote_computer.sidecar_recovery_response_status)"' "$summary_json"
    jq -r '"sidecar_validation_status=\(.remote_computer.sidecar_validation_status)"' "$summary_json"
    jq -r '"sidecar_cluster_id=\(.remote_computer.sidecar_cluster_id)"' "$summary_json"
    jq -r '"sidecar_target_kind=\(.remote_computer.sidecar_target_kind)"' "$summary_json"
    jq -r '"sidecar_node_count=\(.remote_computer.sidecar_node_count)"' "$summary_json"
    jq -r '"sidecar_replacement_scope=\(.remote_computer.sidecar_replacement_scope)"' "$summary_json"
    jq -r '"sidecar_replacement_pods_healthy=\(.remote_computer.replacement_pods_healthy)"' "$summary_json"
    jq -r '"sidecar_checked_pod_count=\(.remote_computer.checked_pod_count)"' "$summary_json"
    jq -r '"sidecar_checked_pod_detail_count=\(.remote_computer.checked_pods | length)"' "$summary_json"
    jq -r '"same_cluster_target=\(.same_cluster_target)"' "$summary_json"
    echo
    echo "real_cluster_blocking_reasons:"
    is_real_cluster_kind "$worker_target_kind" || echo "- worker load validation target is not a real cluster kind: $worker_target_kind"
    is_multi_node "$worker_node_count" || echo "- worker load validation did not report a multi-node cluster: node_count=$worker_node_count"
    is_production_identity "$worker_cluster_id" || echo "- worker load validation cluster id is pilot/mock/local: ${worker_cluster_id:-<empty>}"
    [[ "$worker_load_check_detail_count" =~ ^[0-9]+$ && "$worker_load_check_detail_count" -gt 0 ]] || echo "- worker load validation did not include worker-pool load check details"
    [[ "$state_sync_evidence_status" == "captured" ]] || echo "- Remote Computer state-sync evidence was not captured: $state_sync_evidence_status"
    is_real_cluster_kind "$state_target_kind" || echo "- Remote Computer state-sync target is not a real cluster kind: $state_target_kind"
    is_multi_node "$state_node_count" || echo "- Remote Computer state-sync did not report a multi-node cluster: node_count=$state_node_count"
    is_production_identity "$state_cluster_id" || echo "- Remote Computer state-sync cluster id is pilot/mock/local: ${state_cluster_id:-<empty>}"
    is_distributed_state_backend "$state_backend" || echo "- Remote Computer state backend is not a supported distributed filesystem: $state_backend"
    [[ -n "$state_claim" ]] || echo "- Remote Computer state-sync did not report a state claim"
    [[ "$state_checked_path_count" =~ ^[0-9]+$ && "$state_checked_path_count" -gt 0 ]] || echo "- Remote Computer state-sync did not report any checked state contract paths: checked_path_count=$state_checked_path_count"
    [[ "$state_checked_path_detail_count" =~ ^[0-9]+$ && "$state_checked_path_count" =~ ^[0-9]+$ && "$state_checked_path_detail_count" -ge "$state_checked_path_count" ]] || echo "- Remote Computer state-sync did not include checked path details for every counted path: checked_path_detail_count=$state_checked_path_detail_count checked_path_count=$state_checked_path_count"
    [[ "$same_cluster_target" == "true" ]] || echo "- worker, state-sync, and sidecar evidence do not share the same cluster id"
    if [[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" == "1" ]]; then
      [[ "$sidecar_recovery_evidence_status" == "captured" ]] || echo "- sidecar replacement evidence was not captured: $sidecar_recovery_evidence_status"
      [[ "$sidecar_validation_status" == "validated" ]] || echo "- sidecar replacement validation controller did not validate replacement: $sidecar_validation_status"
      is_real_cluster_kind "$sidecar_target_kind" || echo "- sidecar replacement target is not a real cluster kind: $sidecar_target_kind"
      is_multi_node "$sidecar_node_count" || echo "- sidecar replacement did not report a multi-node cluster: node_count=$sidecar_node_count"
      is_production_identity "$sidecar_cluster_id" || echo "- sidecar replacement cluster id is pilot/mock/local: ${sidecar_cluster_id:-<empty>}"
      [[ "$sidecar_replacement_scope" == "cluster" ]] || echo "- sidecar replacement scope is not cluster-wide: $sidecar_replacement_scope"
      [[ "$sidecar_replacement_pods_healthy" == "true" ]] || echo "- sidecar replacement validation did not report healthy replacement Pods"
      [[ "$sidecar_checked_pod_count" =~ ^[0-9]+$ && "$sidecar_checked_pod_count" -gt 0 ]] || echo "- sidecar replacement validation did not report any checked Pods: checked_pod_count=$sidecar_checked_pod_count"
      [[ "$sidecar_checked_pod_detail_count" =~ ^[0-9]+$ && "$sidecar_checked_pod_count" =~ ^[0-9]+$ && "$sidecar_checked_pod_detail_count" -ge "$sidecar_checked_pod_count" ]] || echo "- sidecar replacement validation did not include checked Pod details for every counted Pod: checked_pod_detail_count=$sidecar_checked_pod_detail_count checked_pod_count=$sidecar_checked_pod_count"
    fi
    jq -r '"evidence_dir=\(.evidence_dir)"' "$summary_json"
  } >"$summary_txt"

  cat "$summary_txt"

  if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
    echo "Worker/Remote Computer evidence gate failed closed; set ALLOW_BLOCKED=1 only for inventory runs." >&2
    exit 1
  fi
}

require_cmd jq
require_cmd curl
mkdir -p "$EVIDENCE_DIR"

worker_stdout="$EVIDENCE_DIR/worker.stdout.log"
worker_stderr="$EVIDENCE_DIR/worker.stderr.log"
remote_stdout="$EVIDENCE_DIR/remote-computer.stdout.log"
remote_stderr="$EVIDENCE_DIR/remote-computer.stderr.log"

run_gate worker ./scripts/worker-evidence-gate.sh "$EVIDENCE_DIR/worker" "$worker_stdout" "$worker_stderr"
run_gate remote-computer ./scripts/remote-computer-evidence-gate.sh "$EVIDENCE_DIR/remote-computer" "$remote_stdout" "$remote_stderr"
write_summary
