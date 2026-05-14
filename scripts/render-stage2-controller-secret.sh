#!/usr/bin/env bash
set -euo pipefail

env_file="${1:-deploy/stage2-evidence/stage2-production-controllers.env.example}"
namespace="${MANDOFORGE_K8S_NAMESPACE:-agent-os}"
secret_name="${MANDOFORGE_STAGE2_CONTROLLER_SECRET_NAME:-mandoforge-stage2-controller-env}"
allow_placeholders="${ALLOW_STAGE2_CONTROLLER_PLACEHOLDERS:-0}"

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to render the Stage 2 controller Secret" >&2
  exit 1
fi

if [[ ! -f "$env_file" ]]; then
  echo "missing Stage 2 controller env file: $env_file" >&2
  exit 1
fi

if [[ "$allow_placeholders" != "1" ]]; then
  if grep -E '(^|=)https://[^[:space:]]*\.example\.com|controller\.example\.com|codex-app-server\.example\.com' "$env_file" >/dev/null; then
    echo "Stage 2 controller env file still contains example.com placeholder URLs" >&2
    exit 1
  fi

  if grep -E '^(MANDOFORGE_KMS_KEY_ID|.*_TOKEN)=$' "$env_file" >/dev/null; then
    echo "Stage 2 controller env file still contains empty KMS key id or token values" >&2
    grep -nE '^(MANDOFORGE_KMS_KEY_ID|.*_TOKEN)=$' "$env_file" >&2
    exit 1
  fi
fi

kubectl create secret generic "$secret_name" \
  --namespace "$namespace" \
  --from-env-file "$env_file" \
  --dry-run=client \
  -o yaml
