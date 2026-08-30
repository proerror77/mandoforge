#!/usr/bin/env bash
set -euo pipefail

admin_evidence_scripts=(
  scripts/policy-rollout-evidence-gate.sh
  scripts/whiskey-adoption-evidence.sh
  scripts/whiskey-remote-computer-state-provider-readiness.sh
)

for script in "${admin_evidence_scripts[@]}"; do
  if [[ ! -f "$script" ]]; then
    echo "missing evidence gate script: $script" >&2
    exit 1
  fi
  if ! grep -Fq 'MANDOFORGE_DEV_ADMIN_TOKEN' "$script"; then
    echo "$script must use MANDOFORGE_DEV_ADMIN_TOKEN for Whiskey evidence auth" >&2
    exit 1
  fi
done

for script in scripts/*.sh; do
  case "$script" in
    scripts/verify-whiskey-evidence-auth-fallbacks.sh)
      continue
      ;;
  esac
  if grep -Fq 'MANDOFORGE_DEV_ADMIN_TOKEN:-${MANDOFORGE_WORKER_TOKEN' "$script" \
    || grep -Fq 'read_env MANDOFORGE_WORKER_TOKEN' "$script"; then
    echo "$script must not use MANDOFORGE_WORKER_TOKEN as an administrative fallback" >&2
    exit 1
  fi
done

for script in scripts/whiskey-adoption-deploy.sh scripts/whiskey-adoption-evidence.sh; do
  if ! grep -Fq 'require_distinct_runtime_tokens' "$script"; then
    echo "$script must reject identical administrative, worker, and scheduler tokens" >&2
    exit 1
  fi
  if ! grep -Fq 'secure_env_file' "$script" \
    || ! grep -Fq 'chmod 0600' "$script" \
    || ! grep -Fq 'must be owned by the deployment user' "$script"; then
    echo "$script must enforce owner-only permissions before loading whiskey.env" >&2
    exit 1
  fi
done

if ! grep -Fq 'chmod 0600 "$REMOTE_ENV"' scripts/whiskey-remote-computer-k3s-cluster-stage.sh \
  || ! grep -Fq 'must be owned by the deployment user' scripts/whiskey-remote-computer-state-provider-readiness.sh; then
  echo "every Whiskey runtime env reader/writer must enforce owner-only permissions" >&2
  exit 1
fi

if grep -Eq 'TOKEN="\$\{[^}]+:-whiskey-|set_env MANDOFORGE_SCHEDULER_TOKEN whiskey-' scripts/whiskey-adoption-deploy.sh; then
  echo "Whiskey deploy must not ship predictable controller or scheduler token defaults" >&2
  exit 1
fi

if grep -Fq 'LOCAL_WEB_DIR' scripts/whiskey-adoption-deploy.sh; then
  echo "Whiskey deploy must use console assets from the pinned image, not the caller checkout" >&2
  exit 1
fi

if ! grep -Fq 'CSP_VALUE="$LIVE_CSP"' scripts/verify-static-ui-assets.sh \
  || ! grep -Fq 'curl -fsS -D "$headers_file"' scripts/verify-static-ui-assets.sh; then
  echo "Whiskey static UI evidence must read the live index, assets, and CSP header" >&2
  exit 1
fi

required_secret_count="$(grep -c '^require_unique_secret WHISKEY_' scripts/whiskey-adoption-deploy.sh)"
if [[ "$required_secret_count" != "13" ]]; then
  echo "Whiskey deploy must validate all 13 independent control-plane secrets" >&2
  exit 1
fi

echo "Whiskey evidence gates keep administrative and worker credentials separate"
