#!/usr/bin/env bash
set -euo pipefail

JUICEFS_ENV_FILE="${1:-deploy/k8s/remote-computer-state-juicefs.env.example}"
RUNTIME_ENV_FILE="${2:-deploy/whiskey/remote-computer-runtime.env.example}"
OUTPUT_DIR="${3:-.mandoforge/remote-adoption/whiskey/remote-computer-unblock-bundle}"

if [[ ! -f "$JUICEFS_ENV_FILE" ]]; then
  echo "missing JuiceFS env file: $JUICEFS_ENV_FILE" >&2
  exit 1
fi

if [[ ! -f "$RUNTIME_ENV_FILE" ]]; then
  echo "missing Remote Computer runtime env file: $RUNTIME_ENV_FILE" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
juicefs_render_script="$script_dir/render-remote-computer-juicefs-profile.sh"
runtime_render_script="$script_dir/render-remote-computer-runtime-env.sh"

if [[ ! -x "$juicefs_render_script" ]]; then
  echo "missing executable helper: $juicefs_render_script" >&2
  exit 1
fi

if [[ ! -x "$runtime_render_script" ]]; then
  echo "missing executable helper: $runtime_render_script" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"

juicefs_yaml="$OUTPUT_DIR/remote-computer-state-juicefs-profile.rendered.yaml"
runtime_env="$OUTPUT_DIR/whiskey-remote-computer-runtime.rendered.env"
checklist_txt="$OUTPUT_DIR/remote-computer-unblock-checklist.txt"

"$juicefs_render_script" "$JUICEFS_ENV_FILE" >"$juicefs_yaml"
"$runtime_render_script" "$RUNTIME_ENV_FILE" >"$runtime_env"

{
  echo "Whiskey Remote Computer Unblock Bundle"
  echo "generated_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "juicefs_env_file=$JUICEFS_ENV_FILE"
  echo "runtime_env_file=$RUNTIME_ENV_FILE"
  echo "juicefs_rendered_yaml=$juicefs_yaml"
  echo "runtime_rendered_env=$runtime_env"
  echo
  echo "apply_sequence:"
  echo "- review rendered files"
  echo "- copy rendered runtime env lines into /opt/mandoforge-adoption/whiskey.env on Whiskey"
  echo "- apply rendered JuiceFS Secret/PV manifest to the cluster"
  echo "- apply deploy/k8s/remote-computer-state-juicefs-pvc-patch.yaml through the pilot kustomization or equivalent patch path"
  echo "- rerun scripts/whiskey-remote-computer-k3s-cluster-stage.sh --apply-manifests --run-evidence"
  echo
  echo "remaining_gates:"
  echo "- live runner mutation must still be validated before sidecar replacement is enabled"
  echo "- distributed Memory/Notes/Skills state sync remains blocked until the state provider and lock manager are real"
} >"$checklist_txt"

printf 'Whiskey Remote Computer Unblock Bundle\n'
printf 'juicefs_rendered_yaml=%s\n' "$juicefs_yaml"
printf 'runtime_rendered_env=%s\n' "$runtime_env"
printf 'checklist=%s\n' "$checklist_txt"
