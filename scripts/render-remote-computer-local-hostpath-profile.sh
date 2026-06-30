#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/k8s/remote-computer-state-local-hostpath.env.example}"

if [[ ! -f "$env_file" ]]; then
  echo "missing local hostPath env file: $env_file" >&2
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

pv_name="${MANDOFORGE_REMOTE_COMPUTER_LOCAL_HOSTPATH_PV_NAME:-mandoforge-remote-computer-state-juicefs-pv}"
host_path="${MANDOFORGE_REMOTE_COMPUTER_LOCAL_HOSTPATH_PATH:-}"
capacity="${MANDOFORGE_REMOTE_COMPUTER_LOCAL_HOSTPATH_CAPACITY:-20Gi}"

if [[ -z "$host_path" ]]; then
  echo "missing required local hostPath value: MANDOFORGE_REMOTE_COMPUTER_LOCAL_HOSTPATH_PATH" >&2
  exit 1
fi

cat <<EOF
apiVersion: v1
kind: PersistentVolume
metadata:
  name: ${pv_name}
  annotations:
    mandoforge.io/profile: remote-computer-state
    mandoforge.io/provider: local-hostpath
    mandoforge.io/scope: single-node-pilot
spec:
  storageClassName: ""
  capacity:
    storage: ${capacity}
  accessModes:
    - ReadWriteMany
  persistentVolumeReclaimPolicy: Retain
  hostPath:
    path: ${host_path}
    type: DirectoryOrCreate
EOF
