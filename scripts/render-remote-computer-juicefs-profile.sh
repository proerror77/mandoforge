#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/k8s/remote-computer-state-juicefs.env.example}"
allow_placeholders="${ALLOW_REMOTE_COMPUTER_JUICEFS_PLACEHOLDERS:-0}"

if [[ ! -f "$env_file" ]]; then
  echo "missing JuiceFS env file: $env_file" >&2
  exit 1
fi

set -a
source "$env_file"
set +a

name="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_NAME:-}"
metaurl="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_METAURL:-}"
storage="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_STORAGE:-}"
bucket="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_BUCKET:-}"
access_key="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_ACCESS_KEY:-}"
secret_key="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_SECRET_KEY:-}"
subpath="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_SUBPATH:-mandoforge/agent-state}"
volume_handle="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_VOLUME_HANDLE:-$name}"
pv_name="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_PV_NAME:-mandoforge-remote-computer-state-juicefs-pv}"
secret_name="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_SECRET_NAME:-mandoforge-remote-computer-juicefs}"
namespace="${MANDOFORGE_REMOTE_COMPUTER_JUICEFS_NAMESPACE:-agent-os}"

require_non_empty() {
  local label="$1"
  local value="$2"
  if [[ -z "$value" ]]; then
    echo "missing required JuiceFS value: $label" >&2
    exit 1
  fi
}

require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_NAME "$name"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_METAURL "$metaurl"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_STORAGE "$storage"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_BUCKET "$bucket"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_ACCESS_KEY "$access_key"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_SECRET_KEY "$secret_key"
require_non_empty MANDOFORGE_REMOTE_COMPUTER_JUICEFS_VOLUME_HANDLE "$volume_handle"

if [[ "$allow_placeholders" != "1" ]]; then
  if [[ "$metaurl" == *"replace-me"* || "$bucket" == *"example.com"* || "$access_key" == "replace-me" || "$secret_key" == "replace-me" ]]; then
    echo "JuiceFS env file still contains placeholder values" >&2
    exit 1
  fi
fi

cat <<EOF
apiVersion: v1
kind: Secret
metadata:
  name: ${secret_name}
  annotations:
    mandoforge.io/profile: remote-computer-state
    mandoforge.io/provider: juicefs
type: Opaque
stringData:
  name: ${name}
  metaurl: ${metaurl}
  storage: ${storage}
  bucket: ${bucket}
  access-key: ${access_key}
  secret-key: ${secret_key}
---
apiVersion: v1
kind: PersistentVolume
metadata:
  name: ${pv_name}
  annotations:
    mandoforge.io/profile: remote-computer-state
    mandoforge.io/provider: juicefs
spec:
  capacity:
    storage: 20Gi
  accessModes:
    - ReadWriteMany
  persistentVolumeReclaimPolicy: Retain
  csi:
    driver: csi.juicefs.com
    volumeHandle: ${volume_handle}
    nodePublishSecretRef:
      name: ${secret_name}
      namespace: ${namespace}
    volumeAttributes:
      subPath: ${subpath}
EOF
