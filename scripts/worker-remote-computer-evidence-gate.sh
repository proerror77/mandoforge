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
  local remote_state_ready
  local state_sync_response_status
  local state_controller_fresh
  local sidecar_recovery_ready
  local sidecar_recovery_response_status
  local runner_ready

  worker_production_ready="$(jq -r '.production_ops.status == "ready" and (.production_ops.production_blocked == false)' "$worker_readiness")"
  isolated_pool="$(jq -r '.load_validation.isolated_worker_pool_configured == true' "$worker_readiness")"
  load_validated="$(jq -r '.load_validation.load_validated == true' "$worker_readiness")"
  worker_controller_fresh="$(jq -r '.load_validation.controller_evidence_fresh == true' "$worker_readiness")"
  worker_validation_status="$(jq -r '.status // "unknown"' "$worker_validation")"
  worker_validation_response_status="$(jq -r '.response.status // "unknown"' "$worker_validation")"

  remote_state_ready="$(jq -r '.production_state_sync.status == "ready" and (.production_state_sync.production_blocked == false)' "$remote_readiness")"
  state_sync_response_status="$(jq -r '.response.status // "unknown"' "$state_sync")"
  state_controller_fresh="$(jq -r '.production_state_sync.controller_evidence_fresh == true' "$remote_readiness")"
  sidecar_recovery_ready="$(jq -r '.sidecar_recovery.status == "ready"' "$remote_readiness")"
  sidecar_recovery_response_status="not_requested"
  if [[ -s "$sidecar_recovery" ]]; then
    sidecar_recovery_response_status="$(jq -r '.response.status // "unknown"' "$sidecar_recovery")"
  fi
  runner_ready="$(jq -r '(.configured == true) and (((.status // "") == "ready") or ((.status // "") == "dry_run_ready") or ((.status // "") == "live_ready"))' "$runner_readiness")"

  local blocked_count=0
  [[ "$worker_production_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$isolated_pool" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$load_validated" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$worker_controller_fresh" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$worker_validation_status" == "captured" ]] || blocked_count=$((blocked_count + 1))
  [[ "$remote_state_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$state_controller_fresh" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$runner_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
  if [[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" == "1" ]]; then
    [[ "$sidecar_recovery_ready" == "true" ]] || blocked_count=$((blocked_count + 1))
    [[ "$sidecar_recovery_response_status" == "validated" || "$sidecar_recovery_response_status" == "recovered" || "$sidecar_recovery_response_status" == "ready" ]] || blocked_count=$((blocked_count + 1))
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
    --arg state_sync_response_status "$state_sync_response_status" \
    --arg sidecar_recovery_response_status "$sidecar_recovery_response_status" \
    --argjson worker_production_ready "$(bool_json "$worker_production_ready")" \
    --argjson isolated_worker_pool_configured "$(bool_json "$isolated_pool")" \
    --argjson load_validated "$(bool_json "$load_validated")" \
    --argjson worker_controller_evidence_fresh "$(bool_json "$worker_controller_fresh")" \
    --argjson remote_state_sync_ready "$(bool_json "$remote_state_ready")" \
    --argjson remote_state_controller_evidence_fresh "$(bool_json "$state_controller_fresh")" \
    --argjson remote_runner_ready "$(bool_json "$runner_ready")" \
    --argjson sidecar_recovery_required "$(bool_json "$([[ "$RUN_STAGE2_REMOTE_SIDECAR_RECOVERY" == "1" ]] && echo true || echo false)")" \
    --argjson sidecar_recovery_ready "$(bool_json "$sidecar_recovery_ready")" \
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
        validation_response_status: $worker_validation_response_status
      },
      remote_computer: {
        readiness_file: $remote_readiness,
        runner_readiness_file: $runner_readiness,
        state_sync_file: $state_sync,
        sidecar_recovery_file: $sidecar_recovery,
        state_sync_ready: $remote_state_sync_ready,
        state_controller_evidence_fresh: $remote_state_controller_evidence_fresh,
        state_sync_response_status: $state_sync_response_status,
        runner_ready: $remote_runner_ready,
        sidecar_recovery_required: $sidecar_recovery_required,
        sidecar_recovery_ready: $sidecar_recovery_ready,
        sidecar_recovery_response_status: $sidecar_recovery_response_status
      }
    }' >"$summary_json"

  {
    jq -r '"worker_remote_computer_status=\(.status)"' "$summary_json"
    jq -r '"production_blocked=\(.production_blocked)"' "$summary_json"
    jq -r '"production_blocked_count=\(.production_blocked_count)"' "$summary_json"
    jq -r '"worker_production_ready=\(.worker.production_ready)"' "$summary_json"
    jq -r '"isolated_worker_pool_configured=\(.worker.isolated_worker_pool_configured)"' "$summary_json"
    jq -r '"worker_load_validated=\(.worker.load_validated)"' "$summary_json"
    jq -r '"worker_controller_evidence_fresh=\(.worker.controller_evidence_fresh)"' "$summary_json"
    jq -r '"remote_state_sync_ready=\(.remote_computer.state_sync_ready)"' "$summary_json"
    jq -r '"remote_state_controller_evidence_fresh=\(.remote_computer.state_controller_evidence_fresh)"' "$summary_json"
    jq -r '"remote_runner_ready=\(.remote_computer.runner_ready)"' "$summary_json"
    jq -r '"sidecar_recovery_required=\(.remote_computer.sidecar_recovery_required)"' "$summary_json"
    jq -r '"sidecar_recovery_ready=\(.remote_computer.sidecar_recovery_ready)"' "$summary_json"
    jq -r '"sidecar_recovery_response_status=\(.remote_computer.sidecar_recovery_response_status)"' "$summary_json"
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
