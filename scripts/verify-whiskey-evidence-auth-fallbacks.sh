#!/usr/bin/env bash
set -euo pipefail

required_scripts=(
  scripts/policy-rollout-evidence-gate.sh
)

for script in "${required_scripts[@]}"; do
  if [[ ! -f "$script" ]]; then
    echo "missing evidence gate script: $script" >&2
    exit 1
  fi
  if ! grep -Fq 'MANDOFORGE_DEV_ADMIN_TOKEN' "$script"; then
    echo "$script must fall back to MANDOFORGE_DEV_ADMIN_TOKEN for Whiskey evidence auth" >&2
    exit 1
  fi
  if ! grep -Fq 'MANDOFORGE_WORKER_TOKEN' "$script"; then
    echo "$script must fall back to MANDOFORGE_WORKER_TOKEN for Whiskey evidence auth" >&2
    exit 1
  fi
done

echo "Whiskey evidence gates have token auth fallbacks"
