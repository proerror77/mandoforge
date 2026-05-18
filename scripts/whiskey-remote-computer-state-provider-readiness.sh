#!/usr/bin/env bash
set -euo pipefail

REMOTE_HOST="${WHISKEY_REMOTE_HOST:-wishky-2-1}"
REMOTE_ROOT="${WHISKEY_REMOTE_ROOT:-/opt/mandoforge-adoption}"
NAMESPACE="${WHISKEY_REMOTE_COMPUTER_NAMESPACE:-agent-os}"
OUTPUT_DIR="${WHISKEY_LOCAL_SYNC_DIR:-.mandoforge/remote-adoption/whiskey}"
OUTPUT_MODE="text"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      REMOTE_HOST="${2:?--host requires a value}"
      shift 2
      ;;
    --remote-root)
      REMOTE_ROOT="${2:?--remote-root requires a value}"
      shift 2
      ;;
    --namespace)
      NAMESPACE="${2:?--namespace requires a value}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:?--output-dir requires a value}"
      shift 2
      ;;
    --json)
      OUTPUT_MODE="json"
      shift
      ;;
    *)
      echo "usage: scripts/whiskey-remote-computer-state-provider-readiness.sh [--host <ssh-host>] [--remote-root <dir>] [--namespace <ns>] [--output-dir <dir>] [--json]" >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "whiskey remote-computer state-provider readiness requires $1" >&2
    exit 1
  fi
}

require_cmd ssh
require_cmd jq

mkdir -p "$OUTPUT_DIR"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
json_file="$OUTPUT_DIR/remote-computer-state-provider-readiness-$stamp.json"
text_file="$OUTPUT_DIR/remote-computer-state-provider-readiness-$stamp.txt"

k3s_verify_output="$(scripts/whiskey-remote-computer-k3s-verify.sh --host "$REMOTE_HOST" --output-dir "$OUTPUT_DIR")"
printf '%s\n' "$k3s_verify_output"
k3s_verify_json="$(printf '%s\n' "$k3s_verify_output" | sed -n 's/^json=//p' | tail -1)"

remote_payload="$(ssh "$REMOTE_HOST" "REMOTE_ROOT='$REMOTE_ROOT' NAMESPACE='$NAMESPACE' bash -s" <<'REMOTE'
set -euo pipefail

remote_root="${REMOTE_ROOT:-/opt/mandoforge-adoption}"
namespace="${NAMESPACE:-agent-os}"
remote_env="$remote_root/whiskey.env"

read_env() {
  local key="$1"
  if [[ -f "$remote_env" ]]; then
    sed -n "s/^${key}=//p" "$remote_env" | tail -n 1
  fi
}

api_port="$(read_env MANDOFORGE_API_HOST_PORT)"
if [[ -z "$api_port" ]]; then
  api_port="18787"
fi

auth_headers=(
  -H "x-mandoforge-subject: whiskey-adoption-admin"
  -H "x-mandoforge-roles: admin"
)

readiness_json="$(curl -fsS "${auth_headers[@]}" "http://127.0.0.1:${api_port}/api/remote-computers/readiness")"
runner_json="$(curl -fsS "${auth_headers[@]}" "http://127.0.0.1:${api_port}/api/remote-computers/runner/readiness")"

secret_json="$(kubectl -n "$namespace" get secret mandoforge-remote-computer-juicefs -o json 2>/dev/null || true)"
pvc_json="$(kubectl -n "$namespace" get pvc mandoforge-remote-computer-state -o json 2>/dev/null || true)"
pv_json="$(kubectl get pv mandoforge-remote-computer-state-juicefs-pv -o json 2>/dev/null || true)"

secret_present=false
placeholder_secret=false
decoded_metaurl=""
decoded_bucket=""
decoded_access_key=""
if [[ -n "$secret_json" ]]; then
  secret_present=true
  decoded_metaurl="$(printf '%s\n' "$secret_json" | jq -r '.data["metaurl"] // ""' | base64 -d 2>/dev/null || true)"
  decoded_bucket="$(printf '%s\n' "$secret_json" | jq -r '.data["bucket"] // ""' | base64 -d 2>/dev/null || true)"
  decoded_access_key="$(printf '%s\n' "$secret_json" | jq -r '.data["access-key"] // ""' | base64 -d 2>/dev/null || true)"
  if [[ "$decoded_metaurl" == *"replace-me"* || "$decoded_bucket" == *"example.com"* || "$decoded_access_key" == "replace-me" ]]; then
    placeholder_secret=true
  fi
fi

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg remote_host "$(hostname)" \
  --arg namespace "$namespace" \
  --arg state_provider_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER)" \
  --arg conflict_policy_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_STATE_CONFLICT_POLICY)" \
  --arg lock_manager_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER)" \
  --arg state_sync_controller_required_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_REQUIRED)" \
  --arg state_sync_controller_url_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_STATE_SYNC_CONTROLLER_URL)" \
  --arg runner_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_RUNNER)" \
  --arg execution_transport_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT)" \
  --arg mutation_enabled_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED)" \
  --arg live_mutation_enabled_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED)" \
  --arg execution_enabled_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED)" \
  --arg sidecar_replacement_enabled_env "$(read_env MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED)" \
  --argjson secret_present "$secret_present" \
  --argjson placeholder_secret "$placeholder_secret" \
  --arg decoded_metaurl "$decoded_metaurl" \
  --arg decoded_bucket "$decoded_bucket" \
  --arg decoded_access_key "$decoded_access_key" \
  --argjson readiness "$readiness_json" \
  --argjson runner "$runner_json" \
  --argjson pvc "$(if [[ -n "$pvc_json" ]]; then printf '%s' "$pvc_json"; else printf '{}'; fi)" \
  --argjson pv "$(if [[ -n "$pv_json" ]]; then printf '%s' "$pv_json"; else printf '{}'; fi)" \
  '{
    generated_at: $generated_at,
    remote_host: $remote_host,
    namespace: $namespace,
    env: {
      state_provider: (if $state_provider_env == "" then null else $state_provider_env end),
      conflict_policy: (if $conflict_policy_env == "" then null else $conflict_policy_env end),
      lock_manager: (if $lock_manager_env == "" then null else $lock_manager_env end),
      state_sync_controller_required: (if $state_sync_controller_required_env == "" then null else $state_sync_controller_required_env end),
      state_sync_controller_url: (if $state_sync_controller_url_env == "" then null else $state_sync_controller_url_env end),
      runner: (if $runner_env == "" then null else $runner_env end),
      execution_transport: (if $execution_transport_env == "" then null else $execution_transport_env end),
      mutation_enabled: (if $mutation_enabled_env == "" then null else $mutation_enabled_env end),
      live_mutation_enabled: (if $live_mutation_enabled_env == "" then null else $live_mutation_enabled_env end),
      execution_enabled: (if $execution_enabled_env == "" then null else $execution_enabled_env end),
      sidecar_replacement_enabled: (if $sidecar_replacement_enabled_env == "" then null else $sidecar_replacement_enabled_env end)
    },
    readiness: $readiness,
    runner: $runner,
    juicefs_secret: {
      present: $secret_present,
      placeholder_values_detected: $placeholder_secret,
      decoded_metaurl: (if $decoded_metaurl == "" then null else $decoded_metaurl end),
      decoded_bucket: (if $decoded_bucket == "" then null else $decoded_bucket end),
      decoded_access_key: (if $decoded_access_key == "" then null else $decoded_access_key end)
    },
    pvc: {
      present: ((($pvc | type) == "object") and (($pvc.metadata.name // "") != "")),
      volume_name: ($pvc.spec.volumeName // null),
      annotations: ($pvc.metadata.annotations // {})
    },
    pv: {
      present: ((($pv | type) == "object") and (($pv.metadata.name // "") != "")),
      csi_driver: ($pv.spec.csi.driver // null),
      annotations: ($pv.metadata.annotations // {})
    }
  }'
REMOTE
)"

printf '%s\n' "$remote_payload" >"$json_file"

jq \
  --slurpfile k3s "$k3s_verify_json" \
  '
  .k3s = ($k3s[0] // {})
  | .status = (
      if .readiness.production_state_sync.status == "ready"
        and .runner.live_mutation_enabled == true
        and .readiness.execution_transport.execution_enabled == true
      then "ready"
      else "blocked"
      end
    )
  | .blocking_reasons = (
      []
      + (if .k3s.status != "ready" then ["k3s cluster is not ready"] else [] end)
      + (if .readiness.state_filesystem.distributed_filesystem_configured != true then ["no real distributed filesystem provider is configured"] else [] end)
      + (if .juicefs_secret.present == true and .juicefs_secret.placeholder_values_detected == true then ["JuiceFS profile still uses placeholder secret values"] else [] end)
      + (if .readiness.state_filesystem.lock_manager_configured != true then ["lock-aware state sync manager is not configured"] else [] end)
      + (if .runner.mode != "kubernetes" then ["runner mode is not kubernetes"] else [] end)
      + (if .runner.mutation_enabled != true then ["runner mutation gate is disabled"] else [] end)
      + (if .runner.live_mutation_enabled != true then ["runner live mutation gate is disabled"] else [] end)
      + (if .readiness.execution_transport.execution_enabled != true then ["remote computer execution transport is not enabled"] else [] end)
      + (if .readiness.production_state_sync.controller_required == true and .readiness.production_state_sync.controller_configured != true then ["state sync controller is required but not configured"] else [] end)
    )
  | .next_actions = (
      []
      + ["render a combined reviewable state-provider bundle with scripts/render-whiskey-remote-computer-unblock-bundle.sh <juicefs-env-file> <runtime-env-file> <output-dir>"]
      + (if .readiness.state_filesystem.distributed_filesystem_configured != true then ["set MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER to juicefs, cephfs, longhorn-rwx, or another real shared-state provider"] else [] end)
      + (if .juicefs_secret.present == true and .juicefs_secret.placeholder_values_detected == true then ["render a non-placeholder JuiceFS Secret/PV manifest with scripts/render-remote-computer-juicefs-profile.sh <env-file>"] else [] end)
      + (if .juicefs_secret.present == true and .juicefs_secret.placeholder_values_detected == true then ["replace placeholder JuiceFS secret values in deploy/k8s/remote-computer-state-juicefs-profile.yaml before reapplying the profile"] else [] end)
      + (if .readiness.state_filesystem.lock_manager_configured != true then ["enable MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER and validate lock-aware shared-write coordination"] else [] end)
      + (if .runner.mode != "kubernetes" or .runner.mutation_enabled != true or .runner.live_mutation_enabled != true or .readiness.execution_transport.execution_enabled != true then ["render the Whiskey runtime env overrides with scripts/render-remote-computer-runtime-env.sh <env-file>"] else [] end)
      + (if .runner.mode != "kubernetes" then ["set MANDOFORGE_REMOTE_COMPUTER_RUNNER=kubernetes"] else [] end)
      + (if .runner.mutation_enabled != true then ["set MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED=true"] else [] end)
      + (if .runner.live_mutation_enabled != true then ["set MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED=true"] else [] end)
      + (if .readiness.execution_transport.execution_enabled != true then ["set MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=kubernetes and MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED=true"] else [] end)
      + (if .env.sidecar_replacement_enabled != "true" then ["enable MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED=true only after live runner mutation is validated"] else [] end)
      + ["rerun scripts/whiskey-remote-computer-k3s-cluster-stage.sh --apply-manifests --run-evidence after the state provider and runner gates are configured"]
    )
  ' "$json_file" >"$json_file.tmp"
mv "$json_file.tmp" "$json_file"

jq -r '
  [
    "Whiskey Remote Computer State Provider Readiness",
    "generated_at=" + .generated_at,
    "remote_host=" + .remote_host,
    "namespace=" + .namespace,
    "status=" + .status,
    "k3s_status=" + (.k3s.status // "unknown"),
    "state_provider=" + (.readiness.state_filesystem.provider // "unknown"),
    "state_filesystem_status=" + (.readiness.state_filesystem.status // "unknown"),
    "production_state_sync_status=" + (.readiness.production_state_sync.status // "unknown"),
    "runner_mode=" + (.runner.mode // "unknown"),
    "runner_status=" + (.runner.status // "unknown"),
    "runner_live_mutation_enabled=" + ((.runner.live_mutation_enabled // false) | tostring),
    "execution_transport_status=" + (.readiness.execution_transport.status // "unknown"),
    "execution_enabled=" + ((.readiness.execution_transport.execution_enabled // false) | tostring),
    "juicefs_secret_present=" + (.juicefs_secret.present | tostring),
    "juicefs_placeholder_values_detected=" + (.juicefs_secret.placeholder_values_detected | tostring),
    "pvc_present=" + (.pvc.present | tostring),
    "pv_present=" + (.pv.present | tostring),
    "",
    "blocking_reasons:",
    (if (.blocking_reasons | length) == 0 then "- none" else (.blocking_reasons[] | "- " + .) end),
    "",
    "next_actions:",
    (if (.next_actions | length) == 0 then "- none" else (.next_actions[] | "- " + .) end)
  ] | flatten | .[]
' "$json_file" >"$text_file"

cp "$json_file" "$OUTPUT_DIR/remote-computer-state-provider-readiness-latest.json"
cp "$text_file" "$OUTPUT_DIR/remote-computer-state-provider-readiness-latest.txt"

if [[ "$OUTPUT_MODE" == "json" ]]; then
  cat "$json_file"
  exit 0
fi

cat "$text_file"
printf '\njson=%s\ntext=%s\n' "$json_file" "$text_file"
