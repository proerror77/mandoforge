#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/k8s/remote-computer-state-local-hostpath.env.example}"

if [[ ! -f "$env_file" ]]; then
  echo "missing local hostPath env file: $env_file" >&2
  exit 1
fi

set -a
source "$env_file"
set +a

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
