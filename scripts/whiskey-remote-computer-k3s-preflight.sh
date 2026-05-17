#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${1:-${WHISKEY_REMOTE_HOST:-wishky-2-1}}"
OUTPUT_DIR="${WHISKEY_REMOTE_COMPUTER_PREFLIGHT_DIR:-.mandoforge/remote-adoption/whiskey}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s preflight requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd jq
require_cmd awk

mkdir -p "$OUTPUT_DIR"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
json_file="$OUTPUT_DIR/remote-computer-k3s-preflight-$stamp.json"
text_file="$OUTPUT_DIR/remote-computer-k3s-preflight-$stamp.txt"

remote_payload="$(ssh "$REMOTE_HOST" 'bash -s' <<'REMOTE'
set -euo pipefail

bool_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    printf 'true'
  else
    printf 'false'
  fi
}

value_or_empty() {
  if command -v "$1" >/dev/null 2>&1; then
    command -v "$1"
  else
    printf ''
  fi
}

mem_total_mib="$(awk "/MemTotal:/ {print int(\$2/1024)}" /proc/meminfo)"
mem_available_mib="$(awk "/MemAvailable:/ {print int(\$2/1024)}" /proc/meminfo)"
swap_total_mib="$(awk "/SwapTotal:/ {print int(\$2/1024)}" /proc/meminfo)"
swap_free_mib="$(awk "/SwapFree:/ {print int(\$2/1024)}" /proc/meminfo)"
swap_used_mib="$((swap_total_mib - swap_free_mib))"
root_avail_gib="$(df -B1 / | awk "NR==2 {printf \"%.1f\", \$4/1024/1024/1024}")"
docker_root_avail_gib="$(df -B1 /var/lib/docker 2>/dev/null | awk "NR==2 {printf \"%.1f\", \$4/1024/1024/1024}")"
opt_avail_gib="$(df -B1 /opt 2>/dev/null | awk "NR==2 {printf \"%.1f\", \$4/1024/1024/1024}")"
cpu_model="$(lscpu 2>/dev/null | awk -F: "/Model name/ {sub(/^[ \t]+/, \"\", \$2); print \$2; exit}")"
virtualization="$(systemd-detect-virt 2>/dev/null || true)"
cgroup_fs="$(stat -fc %T /sys/fs/cgroup)"
bridge_nf_exists="false"
bridge_nf_call_iptables="missing"
if [[ -e /proc/sys/net/bridge/bridge-nf-call-iptables ]]; then
  bridge_nf_exists="true"
  bridge_nf_call_iptables="$(cat /proc/sys/net/bridge/bridge-nf-call-iptables 2>/dev/null || printf 'unknown')"
fi
ip_forward="$(sysctl -n net.ipv4.ip_forward 2>/dev/null || printf 'unknown')"
br_netfilter_loaded="false"
overlay_loaded="false"
if lsmod 2>/dev/null | awk "{print \$1}" | grep -qx br_netfilter; then
  br_netfilter_loaded="true"
fi
if lsmod 2>/dev/null | awk "{print \$1}" | grep -qx overlay; then
  overlay_loaded="true"
fi
reserved_ports_json="$(ss -ltnup 2>/dev/null | awk "/:(6443|10250|8472|2379|2380)\\b/ {print \$5}" | jq -R -s 'split("\n") | map(select(length > 0)) | unique')"
docker_info_json="{}"
if command -v docker >/dev/null 2>&1; then
  docker_info_json="$(docker info --format "{{json .}}" 2>/dev/null | jq '{driver: .Driver, cgroup_driver: .CgroupDriver, cgroup_version: .CgroupVersion, root_dir: .DockerRootDir, mem_total_bytes: .MemTotal, cpu_count: .NCPU, security_options: .SecurityOptions}')"
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg hostname "$(hostname)" \
  --arg kernel "$(uname -r)" \
  --arg cpu_model "$cpu_model" \
  --arg virtualization "$virtualization" \
  --arg cgroup_fs "$cgroup_fs" \
  --arg root_avail_gib "$root_avail_gib" \
  --arg docker_root_avail_gib "${docker_root_avail_gib:-}" \
  --arg opt_avail_gib "${opt_avail_gib:-}" \
  --arg bridge_nf_call_iptables "$bridge_nf_call_iptables" \
  --arg ip_forward "$ip_forward" \
  --argjson cpu_count "$(nproc)" \
  --argjson mem_total_mib "$mem_total_mib" \
  --argjson mem_available_mib "$mem_available_mib" \
  --argjson swap_total_mib "$swap_total_mib" \
  --argjson swap_used_mib "$swap_used_mib" \
  --argjson br_netfilter_loaded "$br_netfilter_loaded" \
  --argjson overlay_loaded "$overlay_loaded" \
  --argjson bridge_nf_exists "$bridge_nf_exists" \
  --argjson binaries "$(jq -n \
    --argjson k3s "$(bool_cmd k3s)" \
    --argjson kubectl "$(bool_cmd kubectl)" \
    --argjson containerd "$(bool_cmd containerd)" \
    --argjson ctr "$(bool_cmd ctr)" \
    --argjson crictl "$(bool_cmd crictl)" \
    --argjson nerdctl "$(bool_cmd nerdctl)" \
    --argjson docker "$(bool_cmd docker)" \
    --argjson iptables "$(bool_cmd iptables)" \
    --argjson nft "$(bool_cmd nft)" \
    --argjson systemctl "$(bool_cmd systemctl)" \
    '{k3s: $k3s, kubectl: $kubectl, containerd: $containerd, ctr: $ctr, crictl: $crictl, nerdctl: $nerdctl, docker: $docker, iptables: $iptables, nft: $nft, systemctl: $systemctl}')" \
  --argjson reserved_ports "$reserved_ports_json" \
  --argjson docker_info "$docker_info_json" \
  '{
    generated_at: $generated_at,
    hostname: $hostname,
    kernel: $kernel,
    cpu_count: $cpu_count,
    cpu_model: $cpu_model,
    virtualization: $virtualization,
    cgroup_fs: $cgroup_fs,
    mem_total_mib: $mem_total_mib,
    mem_available_mib: $mem_available_mib,
    swap_total_mib: $swap_total_mib,
    swap_used_mib: $swap_used_mib,
    root_avail_gib: ($root_avail_gib | tonumber),
    docker_root_avail_gib: (if ($docker_root_avail_gib | length) == 0 then null else ($docker_root_avail_gib | tonumber) end),
    opt_avail_gib: (if ($opt_avail_gib | length) == 0 then null else ($opt_avail_gib | tonumber) end),
    br_netfilter_loaded: $br_netfilter_loaded,
    overlay_loaded: $overlay_loaded,
    bridge_nf_exists: $bridge_nf_exists,
    bridge_nf_call_iptables: $bridge_nf_call_iptables,
    ip_forward: $ip_forward,
    binaries: $binaries,
    reserved_ports: $reserved_ports,
    docker: $docker_info
  }'
REMOTE
)"

printf '%s\n' "$remote_payload" >"$json_file"

jq '
  .warnings = (
    []
    + (if .mem_available_mib < 2048 then ["available memory below 2 GiB"] else [] end)
    + (if .swap_used_mib > 1024 then ["swap usage above 1 GiB"] else [] end)
    + (if .root_avail_gib < 40 then ["root filesystem free space below 40 GiB"] else [] end)
    + (if (.docker_root_avail_gib // 0) < 40 then ["docker root free space below 40 GiB"] else [] end)
    + (if .br_netfilter_loaded != true then ["br_netfilter kernel module is not loaded"] else [] end)
    + (if .bridge_nf_exists != true or .bridge_nf_call_iptables != "1" then ["bridge-nf-call-iptables is not enabled"] else [] end)
    + (if .binaries.k3s != true then ["k3s is not installed"] else [] end)
    + (if .binaries.kubectl != true then ["kubectl is not installed"] else [] end)
  )
  | .blockers = (
    []
    + (if .binaries.systemctl != true then ["systemctl is unavailable"] else [] end)
    + (if .binaries.iptables != true then ["iptables is unavailable"] else [] end)
    + (if (.reserved_ports | length) > 0 then ["k3s-reserved ports are already in use"] else [] end)
  )
  | .status = (
    if (.blockers | length) > 0 then "blocked"
    elif (.warnings | length) > 0 then "constrained_pilot_only"
    else "ready"
    end
  )
  | .recommended_profile = (
    if .status == "blocked" then "do_not_install"
    elif .status == "constrained_pilot_only" then "single-node constrained pilot only"
    else "single-node pilot feasible"
    end
  )
  | .required_actions = (
    []
    + (if .br_netfilter_loaded != true then ["load br_netfilter before installing k3s"] else [] end)
    + (if .bridge_nf_exists != true or .bridge_nf_call_iptables != "1" then ["enable net.bridge.bridge-nf-call-iptables=1 before installing k3s"] else [] end)
    + (if (.reserved_ports | length) > 0 then ["free or remap ports 6443/10250/8472/2379/2380 before installing k3s"] else [] end)
    + (if .mem_available_mib < 2048 then ["cap Remote Computer warm-pool replicas to 0 or 1 and avoid public ingress"] else [] end)
    + (if .swap_used_mib > 1024 then ["reduce host memory pressure before enabling Pod-based Remote Computer execution"] else [] end)
  )
  | .suggested_flags = [
    "server",
    "--disable=traefik",
    "--write-kubeconfig-mode=644",
    "--kube-apiserver-arg=service-node-port-range=30080-30443"
  ]
' "$json_file" >"$json_file.tmp"
mv "$json_file.tmp" "$json_file"

jq -r '
  [
    "Whiskey Remote Computer k3s Preflight",
    "generated_at=" + .generated_at,
    "hostname=" + .hostname,
    "status=" + .status,
    "recommended_profile=" + .recommended_profile,
    "cpu_count=" + (.cpu_count | tostring),
    "mem_total_mib=" + (.mem_total_mib | tostring),
    "mem_available_mib=" + (.mem_available_mib | tostring),
    "swap_total_mib=" + (.swap_total_mib | tostring),
    "swap_used_mib=" + (.swap_used_mib | tostring),
    "root_avail_gib=" + (.root_avail_gib | tostring),
    "docker_root_avail_gib=" + ((.docker_root_avail_gib // "n/a") | tostring),
    "cgroup_fs=" + .cgroup_fs,
    "virtualization=" + (.virtualization // "unknown"),
    "br_netfilter_loaded=" + (.br_netfilter_loaded | tostring),
    "bridge_nf_call_iptables=" + .bridge_nf_call_iptables,
    "ip_forward=" + .ip_forward,
    "reserved_ports=" + (if (.reserved_ports | length) == 0 then "none" else (.reserved_ports | join(",")) end),
    "",
    "warnings:",
    (if (.warnings | length) == 0 then "- none" else (.warnings[] | "- " + .) end),
    "",
    "blockers:",
    (if (.blockers | length) == 0 then "- none" else (.blockers[] | "- " + .) end),
    "",
    "required_actions:",
    (if (.required_actions | length) == 0 then "- none" else (.required_actions[] | "- " + .) end),
    "",
    "suggested_flags:",
    (.suggested_flags[] | "- " + .)
  ] | flatten | .[]
' "$json_file" >"$text_file"

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
