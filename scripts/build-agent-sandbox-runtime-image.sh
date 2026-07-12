#!/usr/bin/env bash
set -euo pipefail

IMAGE="${MANDOFORGE_AGENT_SANDBOX_IMAGE:-ghcr.io/proerror77/mandoforge/mandoforge-agent-sandbox-runtime:0.1.5}"
REPO_ROOT="$(git rev-parse --show-toplevel)"

cd "$REPO_ROOT"

if ! git diff --quiet --; then
  echo "stage tracked changes before building the Agent Sandbox runtime image" >&2
  exit 1
fi

for required_path in Dockerfile.agent-sandbox scripts/build-agent-sandbox-runtime-image.sh; do
  if ! git ls-files --error-unmatch "$required_path" >/dev/null 2>&1; then
    echo "Agent Sandbox image input must be tracked in the Git index: $required_path" >&2
    exit 1
  fi
done

if git ls-files --stage | grep -q '^160000 '; then
  echo "Agent Sandbox image context does not support Git submodules" >&2
  exit 1
fi

source_tree="$(git write-tree)"
context_dir="$(mktemp -d -t mandoforge-agent-sandbox-context.XXXXXX)"
cleanup() { rm -rf "$context_dir"; }
trap cleanup EXIT

git checkout-index --all --force --prefix="$context_dir/"
printf '%s\n' "$source_tree" >"$context_dir/.mandoforge-tracked-context"

docker build \
  --progress=plain \
  --build-arg "MANDOFORGE_SOURCE_TREE=$source_tree" \
  --file "$context_dir/Dockerfile.agent-sandbox" \
  --tag "$IMAGE" \
  "$@" \
  "$context_dir"
