#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"
INSTALL_CHANNEL="${WHISKEY_K3S_CHANNEL:-stable}"
APPLY_HOST_PREREQS=0
INSTALL_K3S=0
SKIP_MANIFEST_RENDER=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      REMOTE_HOST="${2:?--host requires a value}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:?--output-dir requires a value}"
      shift 2
      ;;
    --channel)
      INSTALL_CHANNEL="${2:?--channel requires a value}"
      shift 2
      ;;
    --apply-host-prereqs)
      APPLY_HOST_PREREQS=1
      shift
      ;;
    --install-k3s)
      INSTALL_K3S=1
      shift
      ;;
    --skip-manifest-render)
      SKIP_MANIFEST_RENDER=1
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-remote-computer-k3s-constrained-pilot.sh [--host <ssh-host>] [--output-dir <dir>] [--channel <stable|latest|vX.Y>] [--apply-host-prereqs] [--install-k3s] [--skip-manifest-render]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s constrained pilot requires $1" >&2
    exit 1
  fi
}

require_cmd jq

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
host_inventory_script="$script_dir/whiskey-remote-computer-k3s-host-inventory.sh"
prepare_script="$script_dir/whiskey-remote-computer-k3s-prepare.sh"
install_script="$script_dir/whiskey-remote-computer-k3s-install.sh"
verify_script="$script_dir/whiskey-remote-computer-k3s-verify.sh"

mkdir -p "$OUTPUT_DIR"

read_path_var() {
  local label="$1"
  local text="$2"
  printf '%s\n' "$text" | sed -n "s/^${label}=//p" | tail -1
}

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
plan_json="$OUTPUT_DIR/remote-computer-k3s-constrained-pilot-$stamp.json"
plan_text="$OUTPUT_DIR/remote-computer-k3s-constrained-pilot-$stamp.txt"

inventory_output="$("$host_inventory_script" --host "$REMOTE_HOST" --output-dir "$OUTPUT_DIR")"
printf '%s\n' "$inventory_output"
inventory_json="$(read_path_var inventory_json "$inventory_output")"
inventory_text="$(read_path_var inventory_text "$inventory_output")"

prepare_args=(--host "$REMOTE_HOST" --output-dir "$OUTPUT_DIR")
if [[ "$APPLY_HOST_PREREQS" == "1" ]]; then
  prepare_args=(--apply "${prepare_args[@]}")
fi
prepare_output="$("$prepare_script" "${prepare_args[@]}")"
printf '%s\n' "$prepare_output"
prepare_json="$(read_path_var json "$prepare_output")"
prepare_text="$(read_path_var text "$prepare_output")"

install_args=(--host "$REMOTE_HOST" --output-dir "$OUTPUT_DIR" --channel "$INSTALL_CHANNEL")
if [[ "$INSTALL_K3S" == "1" ]]; then
  install_args=(--apply "${install_args[@]}")
fi
install_output="$("$install_script" "${install_args[@]}")"
printf '%s\n' "$install_output"
install_json="$(read_path_var json "$install_output")"
install_text="$(read_path_var text "$install_output")"

verify_output="$("$verify_script" --host "$REMOTE_HOST" --output-dir "$OUTPUT_DIR")"
printf '%s\n' "$verify_output"
verify_json="$(read_path_var json "$verify_output")"
verify_text="$(read_path_var text "$verify_output")"

manifest_render_status="skipped"
manifest_render_details="kubectl render skipped"
if [[ "$SKIP_MANIFEST_RENDER" != "1" ]]; then
  if command -v kubectl >/dev/null 2>&1; then
    base_render_file="$(mktemp)"
    pilot_render_file="$(mktemp)"
    kubectl kustomize deploy/k8s >"$base_render_file"
    kubectl kustomize deploy/remote-computer-pilot --load-restrictor LoadRestrictionsNone >"$pilot_render_file"
    base_render_lines="$(wc -l <"$base_render_file" | awk '{print $1}')"
    pilot_render_lines="$(wc -l <"$pilot_render_file" | awk '{print $1}')"
    manifest_render_status="rendered"
    manifest_render_details="deploy/k8s lines=$base_render_lines; deploy/remote-computer-pilot lines=$pilot_render_lines"
    rm -f "$base_render_file" "$pilot_render_file"
  else
    manifest_render_status="missing_kubectl"
    manifest_render_details="kubectl unavailable; render skipped"
  fi
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg remote_host "$REMOTE_HOST" \
  --arg install_channel "$INSTALL_CHANNEL" \
  --arg manifest_render_status "$manifest_render_status" \
  --arg manifest_render_details "$manifest_render_details" \
  --arg inventory_json "$inventory_json" \
  --arg inventory_text "$inventory_text" \
  --arg prepare_json "$prepare_json" \
  --arg prepare_text "$prepare_text" \
  --arg install_json "$install_json" \
  --arg install_text "$install_text" \
  --arg verify_json "$verify_json" \
  --arg verify_text "$verify_text" \
  --arg next_stage_command "scripts/whiskey-remote-computer-k3s-cluster-stage.sh --apply-manifests --run-evidence" \
  --arg next_evidence_command "RUN_STAGE2_PRODUCTION_VALIDATIONS=1 scripts/whiskey-adoption-evidence.sh" \
  --arg next_apply_command "scripts/whiskey-remote-computer-k3s-cluster-stage.sh --apply-manifests --run-evidence" \
  --arg next_manifest_apply_command "kubectl apply -k deploy" \
  --argjson apply_host_prereqs "$( [[ "$APPLY_HOST_PREREQS" == "1" ]] && echo true || echo false )" \
  --argjson install_k3s "$( [[ "$INSTALL_K3S" == "1" ]] && echo true || echo false )" \
  --slurpfile inventory "$inventory_json" \
  --slurpfile prepare "$prepare_json" \
  --slurpfile install "$install_json" \
  --slurpfile verify "$verify_json" \
  '{
    generated_at: $generated_at,
    remote_host: $remote_host,
    install_channel: $install_channel,
    apply_host_prereqs: $apply_host_prereqs,
    install_k3s: $install_k3s,
    manifest_render_status: $manifest_render_status,
    manifest_render_details: $manifest_render_details,
    next_stage_command: $next_stage_command,
    next_apply_command: $next_apply_command,
    next_manifest_apply_command: $next_manifest_apply_command,
    next_evidence_command: $next_evidence_command,
    inventory: ($inventory[0] // {}),
    prepare: ($prepare[0] // {}),
    install: ($install[0] // {}),
    verify: ($verify[0] // {}),
    evidence_files: {
      inventory_json: $inventory_json,
      inventory_text: $inventory_text,
      prepare_json: $prepare_json,
      prepare_text: $prepare_text,
      install_json: $install_json,
      install_text: $install_text,
      verify_json: $verify_json,
      verify_text: $verify_text
    }
  }' >"$plan_json"

jq -r '
  [
    "Whiskey Remote Computer k3s Constrained Pilot",
    "generated_at=" + .generated_at,
    "remote_host=" + .remote_host,
    "install_channel=" + .install_channel,
    "apply_host_prereqs=" + (.apply_host_prereqs | tostring),
    "install_k3s=" + (.install_k3s | tostring),
    "inventory_status=" + (.inventory.status // "unknown"),
    "prepare_status=" + (.prepare.status // "unknown"),
    "install_status=" + (.install.status // "unknown"),
    "verify_status=" + (.verify.status // "unknown"),
    "manifest_render_status=" + .manifest_render_status,
    "manifest_render_details=" + .manifest_render_details,
    "",
    "next_stage_command:",
    "- " + .next_stage_command,
    "next_manifest_apply_command:",
    "- " + .next_manifest_apply_command,
    "next_apply_command:",
    "- " + .next_apply_command,
    "next_evidence_command:",
    "- " + .next_evidence_command
  ] | .[]
' "$plan_json" >"$plan_text"

cat "$plan_text"
printf '\njson=%s\ntext=%s\n' "$plan_json" "$plan_text"
