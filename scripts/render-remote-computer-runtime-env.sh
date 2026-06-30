#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/whiskey/remote-computer-runtime.env.example}"
allow_placeholders="${ALLOW_REMOTE_COMPUTER_RUNTIME_PLACEHOLDERS:-0}"

if [[ ! -f "$env_file" ]]; then
  echo "missing Remote Computer runtime env file: $env_file" >&2
  exit 1
fi

trim() {
  printf '%s' "$1" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

load_env_file() {
  local source_file="$1"
  local line
  local name
  local value
  local line_number=0

  while IFS= read -r line || [[ -n "$line" ]]; do
    line_number=$((line_number + 1))
    line="$(trim "$line")"
    [[ -z "$line" || "$line" == \#* ]] && continue
    if [[ "$line" == export[[:space:]]* ]]; then
      line="$(trim "${line#export}")"
    fi
    if [[ "$line" != *=* ]]; then
      echo "$source_file:$line_number must be KEY=value" >&2
      exit 1
    fi
    name="$(trim "${line%%=*}")"
    value="${line#*=}"
    if [[ ! "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      echo "$source_file:$line_number has invalid env var name: ${name:-<empty>}" >&2
      exit 1
    fi
    if [[ "$value" =~ ^\".*\"$ || "$value" =~ ^\'.*\'$ ]]; then
      value="${value:1:${#value}-2}"
    fi
    export "$name=$value"
  done <"$source_file"
}

load_env_file "$env_file"

require_non_empty() {
  local key="$1"
  local value="$2"
  if [[ -z "$value" ]]; then
    echo "missing required Remote Computer runtime value: $key" >&2
    exit 1
  fi
}

require_bool_like() {
  local key="$1"
  local value="$2"
  case "$value" in
    true|false|1|0) ;;
    *)
      echo "Remote Computer runtime value $key must be true/false/1/0" >&2
      exit 1
      ;;
  esac
}

state_provider="${MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER:-}"
conflict_policy="${MANDOFORGE_REMOTE_COMPUTER_STATE_CONFLICT_POLICY:-one-active-writer-per-session}"
lock_manager="${MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER:-}"
runner="${MANDOFORGE_REMOTE_COMPUTER_RUNNER:-}"
namespace="${MANDOFORGE_REMOTE_COMPUTER_NAMESPACE:-agent-os}"
kubeconfig="${MANDOFORGE_REMOTE_COMPUTER_KUBECONFIG:-}"
mutation_enabled="${MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED:-}"
live_mutation_enabled="${MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED:-}"
execution_transport="${MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT:-}"
execution_enabled="${MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED:-}"
sidecar_replacement_enabled="${MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED:-false}"

require_non_empty MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER "$state_provider"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER "$lock_manager"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_RUNNER "$runner"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_NAMESPACE "$namespace"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_KUBECONFIG "$kubeconfig"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED "$mutation_enabled"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED "$live_mutation_enabled"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT "$execution_transport"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED "$execution_enabled"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED "$sidecar_replacement_enabled"

require_bool_like MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER "$lock_manager"
require_bool_like MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED "$mutation_enabled"
require_bool_like MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED "$live_mutation_enabled"
require_bool_like MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED "$execution_enabled"
require_bool_like MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED "$sidecar_replacement_enabled"

if [[ "$allow_placeholders" != "1" ]]; then
  if [[ "$state_provider" == "pvc-placeholder" ]]; then
    echo "Remote Computer runtime env still uses pvc-placeholder" >&2
    exit 1
  fi
  if [[ "$runner" == "reserved" || "$execution_transport" == "reserved" ]]; then
    echo "Remote Computer runtime env still leaves runner or execution transport in reserved mode" >&2
    exit 1
  fi
fi

cat <<EOF
MANDOFORGE_REMOTE_COMPUTER_STATE_PROVIDER=${state_provider}
MANDOFORGE_REMOTE_COMPUTER_STATE_CONFLICT_POLICY=${conflict_policy}
MANDOFORGE_REMOTE_COMPUTER_STATE_LOCK_MANAGER=${lock_manager}
MANDOFORGE_REMOTE_COMPUTER_RUNNER=${runner}
MANDOFORGE_REMOTE_COMPUTER_NAMESPACE=${namespace}
MANDOFORGE_REMOTE_COMPUTER_KUBECONFIG=${kubeconfig}
MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED=${mutation_enabled}
MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED=${live_mutation_enabled}
MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=${execution_transport}
MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED=${execution_enabled}
MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED=${sidecar_replacement_enabled}
EOF
