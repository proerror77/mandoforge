#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR="${EVIDENCE_DIR:-.mandoforge/remote-computer-production-state}"
SOURCE_EVIDENCE_DIR="${SOURCE_EVIDENCE_DIR:-.mandoforge/stage2-production-evidence}"
REMOTE_EVIDENCE_DIR="${REMOTE_COMPUTER_EVIDENCE_DIR:-$SOURCE_EVIDENCE_DIR/remote-computer}"
COMBINED_EVIDENCE_DIR="${WORKER_REMOTE_COMPUTER_EVIDENCE_DIR:-$SOURCE_EVIDENCE_DIR/worker-remote-computer}"
LIFECYCLE_EVIDENCE_FILE="${REMOTE_COMPUTER_SESSION_POD_LIFECYCLE_EVIDENCE_FILE:-$SOURCE_EVIDENCE_DIR/remote-computer-session-pod-lifecycle-evidence.json}"
STATIC_ONLY="${STATIC_ONLY:-0}"
ALLOW_BLOCKED="${ALLOW_BLOCKED:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "remote computer production state gate requires $1" >&2
    exit 1
  fi
}

fail() {
  echo "remote computer production state gate failed: $*" >&2
  exit 1
}

is_multi_node() {
  [[ "$1" =~ ^[0-9]+$ && "$1" -ge 2 ]]
}

is_production_identity() {
  local value
  value="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  [[ -n "$value" ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(whiskey|pilot|mock|example|sample|demo|local|localhost)([./:_-]|$) ]] || return 1
  [[ ! "$value" =~ (^|[./:_-])(127\.0\.0\.1|\[::1\])([./:_-]|$) ]] || return 1
}

is_distributed_state_backend() {
  case "$1" in
    juicefs|cephfs|longhorn-rwx)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

summary_value() {
  local key="$1"
  local file="$2"
  awk -F= -v key="$key" '$1 == key { print substr($0, length(key) + 2); exit }' "$file"
}

static_contract_check() {
  [[ -x scripts/remote-computer-evidence-gate.sh ]] || fail "missing remote-computer evidence gate"
  [[ -x scripts/worker-remote-computer-evidence-gate.sh ]] || fail "missing worker/Remote Computer evidence gate"
  [[ -x scripts/remote-computer-production-state-gate.sh ]] || fail "missing Remote Computer production state gate"

  grep -q "remote-computer-production-state-gate.sh" docs/enterprise-product-completion-contract.md \
    || fail "enterprise completion contract must list the Remote Computer production state gate"
  grep -q "remote-computer-production-state-gate.sh" crates/mandoforge-api/src/enterprise_product_readiness.rs \
    || fail "enterprise readiness must list the Remote Computer production state gate"
  grep -q "remote-computer-production-state-gate.sh" scripts/production-launch-preflight.sh \
    || fail "production launch preflight must run the Remote Computer production state gate"
  grep -q "remote-computer-production-state-gate.sh" deploy/stage2-evidence/remote-computer-production-state-job.example.yaml \
    || fail "Remote Computer production state Job must run the production state gate"
  grep -q "remote-computer-production-state-job" deploy/stage2-production-evidence/kustomization.yaml \
    || fail "Stage 2 production evidence bundle must include the Remote Computer production state Job"

  grep -q "k8s/remote-computer-state-juicefs-profile.yaml" deploy/kustomization.yaml \
    || fail "production Remote Computer bundle must include the JuiceFS state profile"
  grep -q "remote-computer-state-juicefs-pvc-patch.yaml" deploy/kustomization.yaml \
    || fail "production Remote Computer bundle must patch the Pod-mounted state PVC to the distributed PV"
  grep -q "ReadWriteMany" deploy/k8s/remote-computer-state-pvc.yaml \
    || fail "Remote Computer state PVC must request ReadWriteMany"
  grep -q "runtime-api-or-lock-aware-sync" deploy/k8s/remote-computer-state-contract.yaml \
    || fail "Remote Computer state contract must require runtime API or lock-aware sync writes"
  grep -q "one-active-writer-per-session" deploy/k8s/remote-computer-state-contract.yaml \
    || fail "Remote Computer state contract must preserve one active writer per session"
  grep -q "mandoforge-agent-remote-computer-warm-pool" deploy/k8s/remote-computer-warm-pool.yaml \
    || fail "Remote Computer production bundle must include a warm-pool manifest"
  grep -q "mandoforge-remote-computer-artifact-discovery" deploy/k8s/remote-computer-artifact-discovery-sidecar.yaml \
    || fail "Remote Computer artifact discovery sidecar manifest is missing"

  for tracking_key in \
    "mandoforge.io/session-id" \
    "mandoforge.io/remote-computer-id" \
    "mandoforge.io/tenant-id" \
    "mandoforge.io/lease-id" \
    "mandoforge.io/lifecycle"; do
    grep -q "$tracking_key" crates/mandoforge-api/src/remote_computer_runner.rs \
      || fail "Remote Computer runner must stamp Pod tracking metadata: $tracking_key"
  done
  grep -q "REMOTE_COMPUTER_SESSION_POD_LIFECYCLE_EVIDENCE_FILE" deploy/stage2-evidence/remote-computer-production-state-job.example.yaml \
    || fail "Remote Computer production state Job must bind session Pod lifecycle evidence"
}

# Production proof must bind standalone and combined evidence to the same production cluster, state claim, and distributed backend.
write_summary() {
  local status="$1"
  local blocked_count="$2"
  local issue="$3"
  local summary_json="$EVIDENCE_DIR/summary.json"
  local summary_txt="$EVIDENCE_DIR/summary.txt"

  jq -n \
    --arg generated_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --arg status "$status" \
    --arg issue "$issue" \
    --arg remote_evidence_dir "$REMOTE_EVIDENCE_DIR" \
    --arg combined_evidence_dir "$COMBINED_EVIDENCE_DIR" \
    --arg lifecycle_evidence_file "$LIFECYCLE_EVIDENCE_FILE" \
    --argjson blocked_count "$blocked_count" \
    '{
      generated_at: $generated_at,
      status: $status,
      blocked_count: $blocked_count,
      issue: (if $issue == "" then null else $issue end),
      remote_evidence_dir: $remote_evidence_dir,
      combined_evidence_dir: $combined_evidence_dir,
      lifecycle_evidence_file: $lifecycle_evidence_file
    }' >"$summary_json"

  {
    echo "remote_computer_production_state_status=$status"
    echo "blocked_count=$blocked_count"
    if [[ -n "$issue" ]]; then
      echo "issue=$issue"
    fi
    echo "remote_evidence_dir=$REMOTE_EVIDENCE_DIR"
    echo "combined_evidence_dir=$COMBINED_EVIDENCE_DIR"
    echo "lifecycle_evidence_file=$LIFECYCLE_EVIDENCE_FILE"
  } >"$summary_txt"
}

json_string() {
  local query="$1"
  local file="$2"
  jq -r "$query // \"\"" "$file"
}

json_bool() {
  local query="$1"
  local file="$2"
  jq -r "$query // false" "$file"
}

json_int() {
  local query="$1"
  local file="$2"
  jq -r "$query // 0" "$file"
}

require_cmd jq
mkdir -p "$EVIDENCE_DIR"
static_contract_check

if [[ "$STATIC_ONLY" == "1" ]]; then
  write_summary "static_ready" 0 ""
  cat "$EVIDENCE_DIR/summary.txt"
  echo "remote computer production state static gate ok"
  exit 0
fi

remote_summary="$REMOTE_EVIDENCE_DIR/summary.txt"
combined_summary_json="$COMBINED_EVIDENCE_DIR/summary.json"
blocked_count=0
issue=""

if [[ ! -s "$remote_summary" ]]; then
  issue="missing Remote Computer evidence summary: $remote_summary"
  echo "$issue" >&2
  blocked_count=$((blocked_count + 1))
else
  remote_blocked_count="$(summary_value production_blocked_count "$remote_summary")"
  remote_cluster_id="$(summary_value state_sync_cluster_id "$remote_summary")"
  remote_node_count="$(summary_value state_sync_node_count "$remote_summary")"
  remote_backend="$(summary_value state_sync_backend "$remote_summary")"
  remote_state_claim="$(summary_value state_sync_state_claim "$remote_summary")"
  remote_path_detail_count="$(summary_value state_sync_checked_path_detail_count "$remote_summary")"
  sidecar_scope="$(summary_value sidecar_replacement_scope "$remote_summary")"
  sidecar_healthy="$(summary_value sidecar_replacement_pods_healthy "$remote_summary")"
  sidecar_detail_count="$(summary_value sidecar_checked_pod_detail_count "$remote_summary")"

  [[ "$remote_blocked_count" == "0" ]] || blocked_count=$((blocked_count + 1))
  is_production_identity "$remote_cluster_id" || blocked_count=$((blocked_count + 1))
  is_multi_node "$remote_node_count" || blocked_count=$((blocked_count + 1))
  is_distributed_state_backend "$remote_backend" || blocked_count=$((blocked_count + 1))
  [[ -n "$remote_state_claim" ]] || blocked_count=$((blocked_count + 1))
  [[ "$remote_path_detail_count" =~ ^[0-9]+$ && "$remote_path_detail_count" -gt 0 ]] || blocked_count=$((blocked_count + 1))
  [[ "$sidecar_scope" == "cluster" ]] || blocked_count=$((blocked_count + 1))
  [[ "$sidecar_healthy" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$sidecar_detail_count" =~ ^[0-9]+$ && "$sidecar_detail_count" -gt 0 ]] || blocked_count=$((blocked_count + 1))
fi

if [[ ! -s "$combined_summary_json" ]]; then
  issue="${issue:+$issue; }missing worker/Remote Computer combined summary: $combined_summary_json"
  echo "$issue" >&2
  blocked_count=$((blocked_count + 1))
else
  combined_blocked="$(jq -r 'if has("production_blocked") then .production_blocked else true end' "$combined_summary_json")"
  combined_blocked_count="$(jq -r '.production_blocked_count // 1' "$combined_summary_json")"
  combined_cluster_id="$(jq -r '.remote_computer.state_sync_cluster_id // ""' "$combined_summary_json")"
  combined_node_count="$(jq -r '.remote_computer.state_sync_node_count // 0' "$combined_summary_json")"
  combined_backend="$(jq -r '.remote_computer.state_backend // "unknown"' "$combined_summary_json")"
  combined_state_claim="$(jq -r '.remote_computer.state_claim // ""' "$combined_summary_json")"
  worker_cluster_id="$(jq -r '.worker.cluster_id // ""' "$combined_summary_json")"
  worker_pool="$(jq -r '.worker.worker_pool // ""' "$combined_summary_json")"
  same_cluster_target="$(jq -r '.same_cluster_target // false' "$combined_summary_json")"

  [[ "$combined_blocked" == "false" && "$combined_blocked_count" == "0" ]] || blocked_count=$((blocked_count + 1))
  is_production_identity "$combined_cluster_id" || blocked_count=$((blocked_count + 1))
  is_multi_node "$combined_node_count" || blocked_count=$((blocked_count + 1))
  is_distributed_state_backend "$combined_backend" || blocked_count=$((blocked_count + 1))
  [[ -n "$combined_state_claim" ]] || blocked_count=$((blocked_count + 1))
  is_production_identity "$worker_cluster_id" || blocked_count=$((blocked_count + 1))
  [[ -n "$worker_pool" ]] || blocked_count=$((blocked_count + 1))
  [[ "$same_cluster_target" == "true" ]] || blocked_count=$((blocked_count + 1))

  if [[ -n "${remote_cluster_id:-}" && "$combined_cluster_id" != "$remote_cluster_id" ]]; then
    issue="${issue:+$issue; }Remote Computer standalone and combined evidence do not share one cluster id"
    blocked_count=$((blocked_count + 1))
  fi
  if [[ -n "${remote_state_claim:-}" && "$combined_state_claim" != "$remote_state_claim" ]]; then
    issue="${issue:+$issue; }Remote Computer standalone and combined evidence do not share one state claim"
    blocked_count=$((blocked_count + 1))
  fi
  if [[ -n "${remote_backend:-}" && "$combined_backend" != "$remote_backend" ]]; then
    issue="${issue:+$issue; }Remote Computer standalone and combined evidence do not share one distributed backend"
    blocked_count=$((blocked_count + 1))
  fi
fi

if [[ ! -s "$LIFECYCLE_EVIDENCE_FILE" ]]; then
  issue="${issue:+$issue; }missing Remote Computer session Pod lifecycle evidence: $LIFECYCLE_EVIDENCE_FILE"
  echo "$issue" >&2
  blocked_count=$((blocked_count + 1))
else
  lifecycle_cluster_id="$(json_string '.cluster_id' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_session_id="$(json_string '.session_id' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_remote_computer_id="$(json_string '.remote_computer_id' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_lease_id="$(json_string '.lease_id' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_pod_name="$(json_string '.pod_name' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_label_session_id="$(json_string '.pod_labels["mandoforge.io/session-id"]' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_label_remote_computer_id="$(json_string '.pod_labels["mandoforge.io/remote-computer-id"]' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_created="$(json_bool '.live_create.ok // (.live_create.status == "ok")' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_running="$(json_bool '.running // (.pod_phase == "Running")' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_approved_exec="$(json_bool '.approved_exec.ok // (.approved_exec.status == "ok")' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_heartbeat="$(json_bool '.heartbeat.observed // .heartbeat.ok // false' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_release="$(json_bool '.lease_release.ok // (.lease_release.status == "ok")' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_delete="$(json_bool '.pod_delete.ok // (.pod_delete.status == "ok")' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_orphan_sweep="$(json_bool '.orphan_sweep.ok // (.orphan_sweep.status == "ok")' "$LIFECYCLE_EVIDENCE_FILE")"
  lifecycle_orphan_count="$(json_int '.orphan_sweep.orphan_count' "$LIFECYCLE_EVIDENCE_FILE")"

  is_production_identity "$lifecycle_cluster_id" || blocked_count=$((blocked_count + 1))
  [[ -n "$lifecycle_session_id" ]] || blocked_count=$((blocked_count + 1))
  [[ -n "$lifecycle_remote_computer_id" ]] || blocked_count=$((blocked_count + 1))
  [[ -n "$lifecycle_lease_id" ]] || blocked_count=$((blocked_count + 1))
  [[ -n "$lifecycle_pod_name" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_label_session_id" == "$lifecycle_session_id" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_label_remote_computer_id" == "$lifecycle_remote_computer_id" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_created" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_running" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_approved_exec" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_heartbeat" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_release" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_delete" == "true" ]] || blocked_count=$((blocked_count + 1))
  [[ "$lifecycle_orphan_sweep" == "true" && "$lifecycle_orphan_count" == "0" ]] \
    || blocked_count=$((blocked_count + 1))

  if [[ -n "${remote_cluster_id:-}" && "$lifecycle_cluster_id" != "$remote_cluster_id" ]]; then
    issue="${issue:+$issue; }Remote Computer lifecycle and standalone evidence do not share one cluster id"
    blocked_count=$((blocked_count + 1))
  fi
fi

if [[ "$blocked_count" == "0" ]]; then
  write_summary "ready" 0 ""
else
  if [[ -z "$issue" ]]; then
    issue="Remote Computer production state evidence is incomplete or blocked"
  fi
  write_summary "blocked" "$blocked_count" "$issue"
fi

cat "$EVIDENCE_DIR/summary.txt"

if [[ "$blocked_count" != "0" && "$ALLOW_BLOCKED" != "1" ]]; then
  exit 1
fi

echo "remote computer production state gate ok"
