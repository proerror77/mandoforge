#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
LOCAL_DEPLOY_DIR="${WHISKEY_REMOTE_COMPUTER_DEPLOY_DIR:-deploy}"
REMOTE_DEPLOY_DIR="$REMOTE_ROOT/deploy"
LOCAL_SYNC_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
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
    --apply-manifests)
      APPLY_MANIFESTS=1
      shift
      ;;
    --run-evidence)
      RUN_EVIDENCE=1
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-remote-computer-k3s-cluster-stage.sh [--host <ssh-host>] [--remote-root <dir>] [--apply-manifests] [--run-evidence]" >&2
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

verify_output="$(scripts/whiskey-remote-computer-k3s-verify.sh --host "$REMOTE_HOST")"
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

remote_render_file="$LOCAL_SYNC_DIR/remote-computer-k3s-cluster-stage-remote-render-$stamp.txt"
ssh "$REMOTE_HOST" "kubectl kustomize '$REMOTE_DEPLOY_DIR'" >"$remote_render_file"
remote_render_lines="$(wc -l <"$remote_render_file" | awk '{print $1}')"

apply_status="planned"
if [[ "$APPLY_MANIFESTS" == "1" ]]; then
  ssh "$REMOTE_HOST" "kubectl apply -k '$REMOTE_DEPLOY_DIR'"
  apply_status="applied"
fi

evidence_status="not_requested"
if [[ "$RUN_EVIDENCE" == "1" ]]; then
  WHISKEY_WORKFLOW_PACK_MCP_QUERY="${WHISKEY_WORKFLOW_PACK_MCP_QUERY:-README}" \
  RUN_STAGE2_PRODUCTION_VALIDATIONS=1 \
  scripts/whiskey-adoption-evidence.sh
  evidence_status="rerun_completed"
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg remote_host "$REMOTE_HOST" \
  --arg remote_root "$REMOTE_ROOT" \
  --arg remote_deploy_dir "$REMOTE_DEPLOY_DIR" \
  --arg verify_status "$verify_status" \
  --arg apply_status "$apply_status" \
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
    verify_status: $verify_status,
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
    "apply_status=" + .apply_status,
    "evidence_status=" + .evidence_status,
    "local_base_render_lines=" + (.render.local_base_lines | tostring),
    "local_pilot_render_lines=" + (.render.local_pilot_lines | tostring),
    "remote_render_lines=" + (.render.remote_lines | tostring),
    "next_evidence_command=" + .next_evidence_command
  ] | .[]
' "$plan_json" >"$plan_text"

cat "$plan_text"
printf '\njson=%s\ntext=%s\n' "$plan_json" "$plan_text"
