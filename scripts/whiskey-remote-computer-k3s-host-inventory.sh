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
      echo "usage: scripts/whiskey-remote-computer-k3s-host-inventory.sh [--host <ssh-host>] [--output-dir <dir>]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer k3s host inventory requires $1" >&2
    exit 1
  fi
}

require_cmd jq

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
preflight_script="$script_dir/whiskey-remote-computer-k3s-preflight.sh"
verify_script="$script_dir/whiskey-remote-computer-k3s-verify.sh"

mkdir -p "$OUTPUT_DIR"

preflight_output="$("$preflight_script" "$REMOTE_HOST")"
printf '%s\n' "$preflight_output"
preflight_json="$(printf '%s\n' "$preflight_output" | sed -n 's/^json=//p' | tail -1)"
preflight_text="$(printf '%s\n' "$preflight_output" | sed -n 's/^text=//p' | tail -1)"

verify_output="$("$verify_script" --host "$REMOTE_HOST")"
printf '%s\n' "$verify_output"
verify_json="$(printf '%s\n' "$verify_output" | sed -n 's/^json=//p' | tail -1)"
verify_text="$(printf '%s\n' "$verify_output" | sed -n 's/^text=//p' | tail -1)"

for file in "$preflight_json" "$preflight_text" "$verify_json" "$verify_text"; do
  if [[ ! -f "$file" ]]; then
    echo "k3s host inventory expected file is missing: $file" >&2
    exit 1
  fi
done

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
inventory_json="$OUTPUT_DIR/remote-computer-k3s-host-inventory-$stamp.json"
inventory_text="$OUTPUT_DIR/remote-computer-k3s-host-inventory-$stamp.txt"

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg remote_host "$REMOTE_HOST" \
  --arg preflight_json "$preflight_json" \
  --arg preflight_text "$preflight_text" \
  --arg verify_json "$verify_json" \
  --arg verify_text "$verify_text" \
  --slurpfile preflight "$preflight_json" \
  --slurpfile verify "$verify_json" \
  '
  ($preflight[0] // {}) as $preflight
  | ($verify[0] // {}) as $verify
  | {
      generated_at: $generated_at,
      remote_host: $remote_host,
      host: ($preflight.hostname // $verify.hostname // $remote_host),
      status: (
        if ($verify.status // "") == "ready" then "k3s_ready"
        elif ($preflight.status // "") == "blocked" then "blocked"
        elif ($verify.status // "") == "not_installed" then "preinstall_inventory"
        elif ($verify.status // "") == "broken" then "verify_failed"
        else ($verify.status // $preflight.status // "unknown")
        end
      ),
      preflight: $preflight,
      verify: $verify,
      evidence_files: {
        preflight_json: $preflight_json,
        preflight_text: $preflight_text,
        verify_json: $verify_json,
        verify_text: $verify_text
      },
      required_actions: (
        (($preflight.required_actions // []) + ($verify.required_actions // []))
        | map(select(type == "string" and length > 0))
        | unique
      ),
      warnings: (
        (($preflight.warnings // []) + ($preflight.blockers // []) + ($verify.errors // []))
        | map(select(type == "string" and length > 0))
        | unique
      )
    }
  ' >"$inventory_json"

jq -r '
  [
    "Whiskey Remote Computer k3s Host Inventory",
    "generated_at=" + .generated_at,
    "remote_host=" + .remote_host,
    "host=" + .host,
    "status=" + .status,
    "preflight_status=" + (.preflight.status // "unknown"),
    "verify_status=" + (.verify.status // "unknown"),
    "recommended_profile=" + (.preflight.recommended_profile // "unknown"),
    "k3s_installed=" + ((.verify.k3s_installed // false) | tostring),
    "service_active=" + ((.verify.service_active // false) | tostring),
    "node_count=" + ((.verify.node_count // 0) | tostring),
    "ready_node_count=" + ((.verify.ready_node_count // 0) | tostring),
    "mem_available_mib=" + ((.preflight.mem_available_mib // 0) | tostring),
    "swap_used_mib=" + ((.preflight.swap_used_mib // 0) | tostring),
    "br_netfilter_loaded=" + ((.preflight.br_netfilter_loaded // false) | tostring),
    "bridge_nf_call_iptables=" + (.preflight.bridge_nf_call_iptables // "unknown"),
    "",
    "warnings:",
    (if (.warnings | length) == 0 then "- none" else (.warnings[] | "- " + .) end),
    "",
    "required_actions:",
    (if (.required_actions | length) == 0 then "- none" else (.required_actions[] | "- " + .) end)
  ] | flatten | .[]
' "$inventory_json" >"$inventory_text"

cp "$preflight_json" "$OUTPUT_DIR/remote-computer-k3s-preflight-latest.json"
cp "$preflight_text" "$OUTPUT_DIR/remote-computer-k3s-preflight-latest.txt"
cp "$verify_json" "$OUTPUT_DIR/remote-computer-k3s-verify-latest.json"
cp "$verify_text" "$OUTPUT_DIR/remote-computer-k3s-verify-latest.txt"
cp "$inventory_json" "$OUTPUT_DIR/remote-computer-k3s-host-inventory-latest.json"
cp "$inventory_text" "$OUTPUT_DIR/remote-computer-k3s-host-inventory-latest.txt"

cat "$inventory_text"
printf '\npreflight_json=%s\npreflight_text=%s\nverify_json=%s\nverify_text=%s\ninventory_json=%s\ninventory_text=%s\n' \
  "$preflight_json" \
  "$preflight_text" \
  "$verify_json" \
  "$verify_text" \
  "$inventory_json" \
  "$inventory_text"
