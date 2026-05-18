#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"

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
    *)
      echo "usage: scripts/whiskey-remote-computer-k3s-verify.sh [--host <ssh-host>] [--output-dir <dir>]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s verify requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd jq
mkdir -p "$OUTPUT_DIR"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
json_file="$OUTPUT_DIR/remote-computer-k3s-verify-$stamp.json"
text_file="$OUTPUT_DIR/remote-computer-k3s-verify-$stamp.txt"

remote_payload="$(ssh "$REMOTE_HOST" 'bash -s' <<'REMOTE'
set -euo pipefail

bool_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    printf 'true'
  else
    printf 'false'
  fi
}

k3s_installed="$(bool_cmd k3s)"
kubectl_installed="$(bool_cmd kubectl)"
kubeconfig_path="/etc/rancher/k3s/k3s.yaml"
kubeconfig_present="false"
if [[ -s "$kubeconfig_path" ]]; then
  kubeconfig_present="true"
fi
service_active="false"
if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet k3s 2>/dev/null; then
  service_active="true"
fi
reserved_ports_json="$(ss -ltn 2>/dev/null | awk '/:(6443|10250|8472|2379|2380)\b/ {print $4}' | jq -R -s 'split("\n") | map(select(length > 0)) | unique')"
node_json='[]'
kube_system_pods_json='[]'
errors_json='[]'

if [[ "$k3s_installed" == "true" ]]; then
  if ! node_json="$(k3s kubectl get nodes -o json 2>/dev/null)"; then
    errors_json="$(jq -nc --argjson prev "$errors_json" '$prev + ["k3s kubectl get nodes failed"]')"
    node_json='{}'
  fi
  if ! kube_system_pods_json="$(k3s kubectl get pods -n kube-system -o json 2>/dev/null)"; then
    errors_json="$(jq -nc --argjson prev "$errors_json" '$prev + ["k3s kubectl get pods -n kube-system failed"]')"
    kube_system_pods_json='{}'
  fi
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg hostname "$(hostname)" \
  --arg kernel "$(uname -r)" \
  --argjson k3s_installed "$k3s_installed" \
  --argjson kubectl_installed "$kubectl_installed" \
  --argjson kubeconfig_present "$kubeconfig_present" \
  --arg kubeconfig_path "$kubeconfig_path" \
  --argjson service_active "$service_active" \
  --argjson reserved_ports "$reserved_ports_json" \
  --argjson nodes "$node_json" \
  --argjson kube_system_pods "$kube_system_pods_json" \
  --argjson errors "$errors_json" \
  '{
    generated_at: $generated_at,
    hostname: $hostname,
    kernel: $kernel,
    k3s_installed: $k3s_installed,
    kubectl_installed: $kubectl_installed,
    kubeconfig_present: $kubeconfig_present,
    kubeconfig_path: $kubeconfig_path,
    service_active: $service_active,
    reserved_ports: $reserved_ports,
    nodes: $nodes,
    kube_system_pods: $kube_system_pods,
    errors: $errors
  }'
REMOTE
)"

printf '%s\n' "$remote_payload" >"$json_file"

jq '
  .node_count = (
    if (.nodes | type) == "object" then (.nodes.items // [] | length)
    else 0
    end
  )
  | .ready_node_count = (
    if (.nodes | type) == "object" then
      [.nodes.items[]? | select(any(.status.conditions[]?; .type == "Ready" and .status == "True"))] | length
    else 0
    end
  )
  | .kube_system_pod_count = (
    if (.kube_system_pods | type) == "object" then (.kube_system_pods.items // [] | length)
    else 0
    end
  )
  | .running_kube_system_pod_count = (
    if (.kube_system_pods | type) == "object" then
      [.kube_system_pods.items[]? | select(.status.phase == "Running")] | length
    else 0
    end
  )
  | .status = (
    if .k3s_installed != true then "not_installed"
    elif (.errors | length) > 0 then "broken"
    elif .service_active != true then "service_inactive"
    elif .node_count == 0 then "no_nodes"
    elif .ready_node_count < .node_count then "nodes_not_ready"
    else "ready"
    end
  )
  | .required_actions = (
    []
    + (if .k3s_installed != true then ["install k3s before verifying cluster state"] else [] end)
    + (if .service_active != true and .k3s_installed == true then ["start and enable the k3s systemd service"] else [] end)
    + (if .node_count == 0 and .k3s_installed == true then ["check kube-apiserver reachability and kubeconfig permissions"] else [] end)
    + (if .ready_node_count < .node_count and .node_count > 0 then ["wait for the single node to report Ready before Remote Computer validation"] else [] end)
    + (if (.reserved_ports | length) == 0 and .k3s_installed != true then ["reserve 6443/10250/8472/2379/2380 for a future pilot once install is approved"] else [] end)
  )
' "$json_file" >"$json_file.tmp"
mv "$json_file.tmp" "$json_file"

jq -r '
  [
    "Whiskey Remote Computer k3s Verify",
    "generated_at=" + .generated_at,
    "hostname=" + .hostname,
    "status=" + .status,
    "k3s_installed=" + (.k3s_installed | tostring),
    "kubectl_installed=" + (.kubectl_installed | tostring),
    "kubeconfig_present=" + (.kubeconfig_present | tostring),
    "service_active=" + (.service_active | tostring),
    "node_count=" + (.node_count | tostring),
    "ready_node_count=" + (.ready_node_count | tostring),
    "kube_system_pod_count=" + (.kube_system_pod_count | tostring),
    "running_kube_system_pod_count=" + (.running_kube_system_pod_count | tostring),
    "reserved_ports=" + (if (.reserved_ports | length) == 0 then "none" else (.reserved_ports | join(",")) end),
    "",
    "errors:",
    (if (.errors | length) == 0 then "- none" else (.errors[] | "- " + .) end),
    "",
    "required_actions:",
    (if (.required_actions | length) == 0 then "- none" else (.required_actions[] | "- " + .) end)
  ] | flatten | .[]
' "$json_file" >"$text_file"

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
