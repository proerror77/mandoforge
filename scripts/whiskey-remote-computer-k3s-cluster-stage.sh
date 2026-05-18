#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
LOCAL_DEPLOY_DIR="${WHISKEY_REMOTE_COMPUTER_DEPLOY_DIR:-deploy}"
REMOTE_DEPLOY_DIR="$REMOTE_ROOT/deploy"
LOCAL_SYNC_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
KEDA_INSTALL_URL="${WHISKEY_KEDA_INSTALL_URL:-https://github.com/kedacore/keda/releases/download/v2.19.0/keda-2.19.0-core.yaml}"
RENDERED_JUICEFS_PROFILE="${WHISKEY_REMOTE_COMPUTER_JUICEFS_PROFILE:-}"
RENDERED_STATE_PROFILE="${WHISKEY_REMOTE_COMPUTER_STATE_PROFILE:-$RENDERED_JUICEFS_PROFILE}"
RUNTIME_ENV_FILE="${WHISKEY_REMOTE_COMPUTER_RUNTIME_ENV_FILE:-}"
APPLY_MANIFESTS=0
RUN_EVIDENCE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      REMOTE_HOST="${2:?--host requires a value}"
      shift 2
      ;;
    --remote-root)
      REMOTE_ROOT="${2:?--remote-root requires a value}"
      REMOTE_DEPLOY_DIR="${2}/deploy"
      shift 2
      ;;
    --sync-dir)
      LOCAL_SYNC_DIR="${2:?--sync-dir requires a value}"
      shift 2
      ;;
    --juicefs-profile)
      RENDERED_STATE_PROFILE="${2:?--juicefs-profile requires a value}"
      shift 2
      ;;
    --state-profile)
      RENDERED_STATE_PROFILE="${2:?--state-profile requires a value}"
      shift 2
      ;;
    --runtime-env-file)
      RUNTIME_ENV_FILE="${2:?--runtime-env-file requires a value}"
      shift 2
      ;;
    --apply-manifests)
      APPLY_MANIFESTS=1
      shift
      ;;
    --run-evidence)
      RUN_EVIDENCE=1
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-remote-computer-k3s-cluster-stage.sh [--host <ssh-host>] [--remote-root <dir>] [--sync-dir <dir>] [--state-profile <path>] [--juicefs-profile <path>] [--runtime-env-file <path>] [--apply-manifests] [--run-evidence]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s cluster stage requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd rsync
require_cmd jq
require_cmd kubectl

if [[ -n "$RENDERED_STATE_PROFILE" && ! -f "$RENDERED_STATE_PROFILE" ]]; then
  echo "rendered state profile not found: $RENDERED_STATE_PROFILE" >&2
  exit 1
fi

if [[ -n "$RUNTIME_ENV_FILE" && ! -f "$RUNTIME_ENV_FILE" ]]; then
  echo "runtime env file not found: $RUNTIME_ENV_FILE" >&2
  exit 1
fi

ensure_remote_keda() {
  local remote_host="$1"
  local install_url="$2"

  if ssh "$remote_host" "kubectl get crd scaledobjects.keda.sh >/dev/null 2>&1"; then
    echo "Whiskey remote-computer k3s cluster stage: KEDA CRD already present"
    return 0
  fi

  echo "Whiskey remote-computer k3s cluster stage: installing KEDA from $install_url"
  ssh "$remote_host" "kubectl apply --server-side -f '$install_url'"
  ssh "$remote_host" "kubectl wait --for=condition=Available deployment/keda-operator -n keda --timeout=180s"
}

merge_remote_runtime_env() {
  local remote_host="$1"
  local remote_env="$2"
  local runtime_env_file="$3"
  local remote_tmp="$REMOTE_ROOT/remote-computer-runtime-overrides.env"

  rsync -az "$runtime_env_file" "$remote_host:$remote_tmp"
  ssh "$remote_host" "REMOTE_ENV='$remote_env' REMOTE_TMP='$remote_tmp' bash -s" <<'REMOTE'
set -euo pipefail

touch "$REMOTE_ENV"
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  key="${line%%=*}"
  value="${line#*=}"
  if grep -q "^${key}=" "$REMOTE_ENV"; then
    sed -i "s#^${key}=.*#${key}=${value}#" "$REMOTE_ENV"
  else
    printf '%s\n' "${key}=${value}" >>"$REMOTE_ENV"
  fi
done <"$REMOTE_TMP"
rm -f "$REMOTE_TMP"
REMOTE
}

verify_output="$(scripts/whiskey-remote-computer-k3s-verify.sh --host "$REMOTE_HOST" --output-dir "$LOCAL_SYNC_DIR")"
printf '%s\n' "$verify_output"
verify_json="$(printf '%s\n' "$verify_output" | sed -n 's/^json=//p' | tail -1)"
verify_status="$(jq -r '.status // "unknown"' "$verify_json")"

if [[ "$verify_status" != "ready" ]]; then
  echo "Whiskey remote-computer k3s cluster stage requires a ready k3s host; current status is $verify_status" >&2
  exit 1
fi

base_render_file="$(mktemp)"
pilot_render_file="$(mktemp)"
kubectl kustomize deploy/k8s >"$base_render_file"
kubectl kustomize deploy >"$pilot_render_file"
base_render_lines="$(wc -l <"$base_render_file" | awk '{print $1}')"
pilot_render_lines="$(wc -l <"$pilot_render_file" | awk '{print $1}')"
rm -f "$base_render_file" "$pilot_render_file"

mkdir -p "$LOCAL_SYNC_DIR"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
plan_json="$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-$stamp.json"
plan_text="$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-$stamp.txt"

ssh "$REMOTE_HOST" "mkdir -p '$REMOTE_DEPLOY_DIR'"
rsync -az "$LOCAL_DEPLOY_DIR/" "$REMOTE_HOST:$REMOTE_DEPLOY_DIR/"

if [[ -n "$RENDERED_STATE_PROFILE" ]]; then
  rsync -az "$RENDERED_STATE_PROFILE" "$REMOTE_HOST:$REMOTE_DEPLOY_DIR/k8s/remote-computer-state-juicefs-profile.yaml"
fi

if [[ -n "$RUNTIME_ENV_FILE" && ( "$APPLY_MANIFESTS" == "1" || "$RUN_EVIDENCE" == "1" ) ]]; then
  merge_remote_runtime_env "$REMOTE_HOST" "$REMOTE_ROOT/whiskey.env" "$RUNTIME_ENV_FILE"
fi

remote_render_file="$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-remote-render-$stamp.txt"
ssh "$REMOTE_HOST" "kubectl kustomize '$REMOTE_DEPLOY_DIR'" >"$remote_render_file"
remote_render_lines="$(wc -l <"$remote_render_file" | awk '{print $1}')"
apply_dry_run_file="$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-apply-dry-run-$stamp.txt"

apply_status="planned"
apply_dry_run_status="not_requested"
if [[ "$APPLY_MANIFESTS" == "1" ]]; then
  ensure_remote_keda "$REMOTE_HOST" "$KEDA_INSTALL_URL"
  if ssh "$REMOTE_HOST" "kubectl apply --dry-run=server -k '$REMOTE_DEPLOY_DIR'" >"$apply_dry_run_file" 2>&1; then
    apply_dry_run_status="passed"
    ssh "$REMOTE_HOST" "kubectl apply -k '$REMOTE_DEPLOY_DIR'"
    apply_status="applied"
  else
    apply_dry_run_status="failed"
    apply_status="blocked"
  fi
fi

evidence_status="not_requested"
if [[ "$RUN_EVIDENCE" == "1" && "$apply_status" != "blocked" ]]; then
  WHISKEY_WORKFLOW_PACK_MCP_QUERY="${WHISKEY_WORKFLOW_PACK_MCP_QUERY:-README}" \
  WHISKEY_REMOTE_HOST="$REMOTE_HOST" \
  WHISKEY_REMOTE_ROOT="$REMOTE_ROOT" \
  WHISKEY_LOCAL_SYNC_DIR="$LOCAL_SYNC_DIR" \
  RUN_STAGE2_PRODUCTION_VALIDATIONS=1 \
  scripts/whiskey-adoption-evidence.sh
  evidence_status="rerun_completed"
elif [[ "$RUN_EVIDENCE" == "1" && "$apply_status" == "blocked" ]]; then
  evidence_status="blocked_by_apply_dry_run"
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg remote_host "$REMOTE_HOST" \
  --arg remote_root "$REMOTE_ROOT" \
  --arg remote_deploy_dir "$REMOTE_DEPLOY_DIR" \
  --arg rendered_state_profile "${RENDERED_STATE_PROFILE:-}" \
  --arg runtime_env_file "${RUNTIME_ENV_FILE:-}" \
  --arg verify_status "$verify_status" \
  --arg apply_status "$apply_status" \
  --arg apply_dry_run_status "$apply_dry_run_status" \
  --arg apply_dry_run_file "$apply_dry_run_file" \
  --arg evidence_status "$evidence_status" \
  --argjson base_render_lines "$base_render_lines" \
  --argjson pilot_render_lines "$pilot_render_lines" \
  --argjson remote_render_lines "$remote_render_lines" \
  --arg next_evidence_command "WHISKEY_WORKFLOW_PACK_MCP_QUERY=README RUN_STAGE2_PRODUCTION_VALIDATIONS=1 scripts/whiskey-adoption-evidence.sh" \
  '{
    generated_at: $generated_at,
    remote_host: $remote_host,
    remote_root: $remote_root,
    remote_deploy_dir: $remote_deploy_dir,
    rendered_state_profile: (if $rendered_state_profile == "" then null else $rendered_state_profile end),
    runtime_env_file: (if $runtime_env_file == "" then null else $runtime_env_file end),
    verify_status: $verify_status,
    apply_dry_run_status: $apply_dry_run_status,
    apply_dry_run_file: (if $apply_dry_run_status == "not_requested" then null else $apply_dry_run_file end),
    apply_status: $apply_status,
    evidence_status: $evidence_status,
    render: {
      local_base_lines: $base_render_lines,
      local_pilot_lines: $pilot_render_lines,
      remote_lines: $remote_render_lines
    },
    next_evidence_command: $next_evidence_command
  }' >"$plan_json"

jq -r '
  [
    "Whiskey Remote Computer k3s Cluster Stage",
    "generated_at=" + .generated_at,
    "remote_host=" + .remote_host,
    "verify_status=" + .verify_status,
    "rendered_state_profile=" + (.rendered_state_profile // "none"),
    "runtime_env_file=" + (.runtime_env_file // "none"),
    "apply_dry_run_status=" + .apply_dry_run_status,
    "apply_dry_run_file=" + (.apply_dry_run_file // "none"),
    "apply_status=" + .apply_status,
    "evidence_status=" + .evidence_status,
    "local_base_render_lines=" + (.render.local_base_lines | tostring),
    "local_pilot_render_lines=" + (.render.local_pilot_lines | tostring),
    "remote_render_lines=" + (.render.remote_lines | tostring),
    "next_evidence_command=" + .next_evidence_command
  ] | .[]
' "$plan_json" >"$plan_text"

cp "$plan_json" "$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-latest.json"
cp "$plan_text" "$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-latest.txt"
cp "$remote_render_file" "$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-remote-render-latest.txt"
if [[ "$apply_dry_run_status" != "not_requested" ]]; then
  cp "$apply_dry_run_file" "$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-apply-dry-run-latest.txt"
fi

cat "$plan_text"
printf '\njson=%s\ntext=%s\n' "$plan_json" "$plan_text"

if [[ "$apply_status" == "blocked" ]]; then
  exit 1
fi
