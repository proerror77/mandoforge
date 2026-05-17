#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"
MODE="dry_run"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)
      MODE="apply"
      shift
      ;;
    --host)
      REMOTE_HOST="${2:?--host requires a value}"
      shift 2
      ;;
    *)
      echo "usage: scripts/whiskey-remote-computer-k3s-prepare.sh [--apply] [--host <ssh-host>]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s prepare requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd jq
mkdir -p "$OUTPUT_DIR"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
json_file="$OUTPUT_DIR/remote-computer-k3s-prepare-$stamp.json"
text_file="$OUTPUT_DIR/remote-computer-k3s-prepare-$stamp.txt"

remote_payload="$(ssh "$REMOTE_HOST" "MODE='$MODE' bash -s" <<'REMOTE'
set -euo pipefail

mode="${MODE:-dry_run}"
modules_file="/etc/modules-load.d/mandoforge-remote-computer.conf"
sysctl_file="/etc/sysctl.d/99-mandoforge-remote-computer.conf"
modules_line="br_netfilter"
sysctl_lines=$'net.bridge.bridge-nf-call-iptables = 1\nnet.ipv4.ip_forward = 1'

current_bridge_value="missing"
if [[ -e /proc/sys/net/bridge/bridge-nf-call-iptables ]]; then
  current_bridge_value="$(cat /proc/sys/net/bridge/bridge-nf-call-iptables 2>/dev/null || printf 'unknown')"
fi
current_ip_forward="$(sysctl -n net.ipv4.ip_forward 2>/dev/null || printf 'unknown')"
module_loaded="false"
if lsmod 2>/dev/null | awk '{print $1}' | grep -qx br_netfilter; then
  module_loaded="true"
fi
modules_file_present="false"
if [[ -s "$modules_file" ]] && grep -qx "$modules_line" "$modules_file" 2>/dev/null; then
  modules_file_present="true"
fi
sysctl_file_present="false"
if [[ -s "$sysctl_file" ]] && grep -q '^net\.bridge\.bridge-nf-call-iptables = 1$' "$sysctl_file" 2>/dev/null; then
  sysctl_file_present="true"
fi

actions_json='[]'
result_status="planned"
changed=false

if [[ "$mode" == "apply" ]]; then
  if [[ "$module_loaded" != "true" ]]; then
    modprobe br_netfilter
    changed=true
    module_loaded="true"
    actions_json="$(jq -nc --argjson prev "$actions_json" '$prev + ["modprobe br_netfilter"]')"
  fi
  if [[ "$modules_file_present" != "true" ]]; then
    printf '%s\n' "$modules_line" >"$modules_file"
    changed=true
    modules_file_present="true"
    actions_json="$(jq -nc --argjson prev "$actions_json" --arg path "$modules_file" '$prev + ["write " + $path]')"
  fi
  desired_sysctl="$(printf '%s\n' "$sysctl_lines")"
  current_sysctl_file="$(cat "$sysctl_file" 2>/dev/null || printf '')"
  if [[ "$current_sysctl_file" != "$desired_sysctl" ]]; then
    printf '%s\n' "$desired_sysctl" >"$sysctl_file"
    changed=true
    sysctl_file_present="true"
    actions_json="$(jq -nc --argjson prev "$actions_json" --arg path "$sysctl_file" '$prev + ["write " + $path]')"
  fi
  sysctl --system >/dev/null
  current_bridge_value="$(sysctl -n net.bridge.bridge-nf-call-iptables 2>/dev/null || printf 'unknown')"
  current_ip_forward="$(sysctl -n net.ipv4.ip_forward 2>/dev/null || printf 'unknown')"
  actions_json="$(jq -nc --argjson prev "$actions_json" '$prev + ["sysctl --system"]')"
  result_status="applied"
else
  if [[ "$module_loaded" != "true" ]]; then
    actions_json="$(jq -nc --argjson prev "$actions_json" '$prev + ["would run modprobe br_netfilter"]')"
  fi
  if [[ "$modules_file_present" != "true" ]]; then
    actions_json="$(jq -nc --argjson prev "$actions_json" --arg path "$modules_file" '$prev + ["would write " + $path]')"
  fi
  if [[ "$sysctl_file_present" != "true" || "$current_bridge_value" != "1" || "$current_ip_forward" != "1" ]]; then
    actions_json="$(jq -nc --argjson prev "$actions_json" --arg path "$sysctl_file" '$prev + ["would write " + $path, "would run sysctl --system"]')"
  fi
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg mode "$mode" \
  --arg status "$result_status" \
  --arg hostname "$(hostname)" \
  --arg modules_file "$modules_file" \
  --arg sysctl_file "$sysctl_file" \
  --argjson module_loaded "$module_loaded" \
  --argjson modules_file_present "$modules_file_present" \
  --argjson sysctl_file_present "$sysctl_file_present" \
  --arg bridge_value "$current_bridge_value" \
  --arg ip_forward "$current_ip_forward" \
  --argjson changed "$changed" \
  --argjson actions "$actions_json" \
  '{
    generated_at: $generated_at,
    mode: $mode,
    status: $status,
    hostname: $hostname,
    module_loaded: $module_loaded,
    modules_file: $modules_file,
    modules_file_present: $modules_file_present,
    sysctl_file: $sysctl_file,
    sysctl_file_present: $sysctl_file_present,
    bridge_nf_call_iptables: $bridge_value,
    ip_forward: $ip_forward,
    changed: $changed,
    actions: $actions
  }'
REMOTE
)"

printf '%s\n' "$remote_payload" >"$json_file"

jq -r '
  [
    "Whiskey Remote Computer k3s Prepare",
    "generated_at=" + .generated_at,
    "hostname=" + .hostname,
    "mode=" + .mode,
    "status=" + .status,
    "module_loaded=" + (.module_loaded | tostring),
    "modules_file=" + .modules_file,
    "modules_file_present=" + (.modules_file_present | tostring),
    "sysctl_file=" + .sysctl_file,
    "sysctl_file_present=" + (.sysctl_file_present | tostring),
    "bridge_nf_call_iptables=" + .bridge_nf_call_iptables,
    "ip_forward=" + .ip_forward,
    "changed=" + (.changed | tostring),
    "",
    "actions:",
    (if (.actions | length) == 0 then "- none" else (.actions[] | "- " + .) end)
  ] | flatten | .[]
' "$json_file" >"$text_file"

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
