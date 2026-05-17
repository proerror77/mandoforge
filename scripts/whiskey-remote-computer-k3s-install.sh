#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"
MODE="dry_run"
INSTALL_CHANNEL="${WHISKEY_K3S_CHANNEL:-stable}"

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
    --channel)
      INSTALL_CHANNEL="${2:?--channel requires a value}"
      shift 2
      ;;
    *)
      echo "usage: scripts/whiskey-remote-computer-k3s-install.sh [--apply] [--host <ssh-host>] [--channel <stable|latest|vX.Y>]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s install requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd jq
mkdir -p "$OUTPUT_DIR"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
json_file="$OUTPUT_DIR/remote-computer-k3s-install-$stamp.json"
text_file="$OUTPUT_DIR/remote-computer-k3s-install-$stamp.txt"

remote_payload="$(ssh "$REMOTE_HOST" "MODE='$MODE' INSTALL_CHANNEL='$INSTALL_CHANNEL' bash -s" <<'REMOTE'
set -euo pipefail

mode="${MODE:-dry_run}"
channel="${INSTALL_CHANNEL:-stable}"
service_name="k3s"
install_flags=(
  "server"
  "--disable=traefik"
  "--write-kubeconfig-mode=644"
  "--kube-apiserver-arg=service-node-port-range=30080-30443"
)
install_exec="${install_flags[*]}"

bool_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    printf 'true'
  else
    printf 'false'
  fi
}

module_loaded="false"
if lsmod 2>/dev/null | awk '{print $1}' | grep -qx br_netfilter; then
  module_loaded="true"
fi
bridge_nf_call_iptables="missing"
if [[ -e /proc/sys/net/bridge/bridge-nf-call-iptables ]]; then
  bridge_nf_call_iptables="$(cat /proc/sys/net/bridge/bridge-nf-call-iptables 2>/dev/null || printf 'unknown')"
fi
ip_forward="$(sysctl -n net.ipv4.ip_forward 2>/dev/null || printf 'unknown')"
k3s_installed="$(bool_cmd k3s)"
kubectl_installed="$(bool_cmd kubectl)"
service_active="false"
if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet "$service_name" 2>/dev/null; then
  service_active="true"
fi

actions_json='[]'
status="planned"
installed_now="false"
verify_stdout=""

if [[ "$mode" == "apply" ]]; then
  if [[ "$module_loaded" != "true" ]]; then
    echo "br_netfilter must be loaded before installing k3s; run scripts/whiskey-remote-computer-k3s-prepare.sh --apply first" >&2
    exit 1
  fi
  if [[ "$bridge_nf_call_iptables" != "1" ]]; then
    echo "net.bridge.bridge-nf-call-iptables must equal 1 before installing k3s; run scripts/whiskey-remote-computer-k3s-prepare.sh --apply first" >&2
    exit 1
  fi
  if [[ "$k3s_installed" != "true" ]]; then
    curl -sfL https://get.k3s.io | INSTALL_K3S_CHANNEL="$channel" INSTALL_K3S_EXEC="$install_exec" sh -
    installed_now="true"
    k3s_installed="true"
    actions_json="$(jq -nc --argjson prev "$actions_json" --arg channel "$channel" --arg exec "$install_exec" '$prev + ["install k3s channel=" + $channel + " exec=" + $exec]')"
  else
    actions_json="$(jq -nc --argjson prev "$actions_json" '$prev + ["k3s already installed; skipped installer"]')"
  fi
  if command -v systemctl >/dev/null 2>&1; then
    systemctl enable --now "$service_name"
    service_active="true"
    actions_json="$(jq -nc --argjson prev "$actions_json" '$prev + ["systemctl enable --now k3s"]')"
  fi
  verify_stdout="$(k3s kubectl get nodes -o wide 2>/dev/null || true)"
  status="applied"
else
  actions_json="$(jq -nc --argjson prev "$actions_json" --arg channel "$channel" --arg exec "$install_exec" '$prev + ["would run curl -sfL https://get.k3s.io | INSTALL_K3S_CHANNEL=" + $channel + " INSTALL_K3S_EXEC=\"" + $exec + "\" sh -"]')"
  if command -v systemctl >/dev/null 2>&1; then
    actions_json="$(jq -nc --argjson prev "$actions_json" '$prev + ["would run systemctl enable --now k3s"]')"
  fi
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg mode "$mode" \
  --arg status "$status" \
  --arg hostname "$(hostname)" \
  --arg install_channel "$channel" \
  --arg install_exec "$install_exec" \
  --arg bridge_nf_call_iptables "$bridge_nf_call_iptables" \
  --arg ip_forward "$ip_forward" \
  --arg verify_stdout "$verify_stdout" \
  --argjson module_loaded "$module_loaded" \
  --argjson k3s_installed "$k3s_installed" \
  --argjson kubectl_installed "$kubectl_installed" \
  --argjson service_active "$service_active" \
  --argjson installed_now "$installed_now" \
  --argjson actions "$actions_json" \
  '{
    generated_at: $generated_at,
    mode: $mode,
    status: $status,
    hostname: $hostname,
    install_channel: $install_channel,
    install_exec: $install_exec,
    module_loaded: $module_loaded,
    bridge_nf_call_iptables: $bridge_nf_call_iptables,
    ip_forward: $ip_forward,
    k3s_installed: $k3s_installed,
    kubectl_installed: $kubectl_installed,
    service_active: $service_active,
    installed_now: $installed_now,
    verify_stdout: $verify_stdout,
    actions: $actions
  }'
REMOTE
)"

printf '%s\n' "$remote_payload" >"$json_file"

jq -r '
  [
    "Whiskey Remote Computer k3s Install",
    "generated_at=" + .generated_at,
    "hostname=" + .hostname,
    "mode=" + .mode,
    "status=" + .status,
    "install_channel=" + .install_channel,
    "install_exec=" + .install_exec,
    "module_loaded=" + (.module_loaded | tostring),
    "bridge_nf_call_iptables=" + .bridge_nf_call_iptables,
    "ip_forward=" + .ip_forward,
    "k3s_installed=" + (.k3s_installed | tostring),
    "kubectl_installed=" + (.kubectl_installed | tostring),
    "service_active=" + (.service_active | tostring),
    "installed_now=" + (.installed_now | tostring),
    "",
    "actions:",
    (if (.actions | length) == 0 then "- none" else (.actions[] | "- " + .) end),
    "",
    "verify_stdout:",
    (if (.verify_stdout | length) == 0 then "- none" else .verify_stdout end)
  ] | flatten | .[]
' "$json_file" >"$text_file"

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
