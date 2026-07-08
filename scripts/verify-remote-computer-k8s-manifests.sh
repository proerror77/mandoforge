#!/usr/bin/env bash
set -euo pipefail

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to verify Remote Computer manifests" >&2
  exit 1
fi

dry_run_manifests=(
  deploy/k8s/agent-remote-computer.yaml
  deploy/k8s/remote-computer-state-pvc.yaml
  deploy/k8s/remote-computer-state-contract.yaml
  deploy/k8s/remote-computer-state-juicefs-profile.yaml
  deploy/k8s/remote-computer-warm-pool.yaml
  deploy/k8s/agent-sandbox-runtime.yaml
)
runner_source="crates/mandoforge-api/src/remote_computer_runner.rs"

for manifest in "${dry_run_manifests[@]}"; do
  if [[ ! -f "$manifest" ]]; then
    echo "missing Remote Computer manifest: $manifest" >&2
    exit 1
  fi
done

k8s_render="$(mktemp -t mandoforge-k8s-render.XXXXXX.out)"
default_render="$(mktemp -t mandoforge-default-render.XXXXXX.out)"
pilot_render="$(mktemp -t mandoforge-remote-computer-pilot-render.XXXXXX.out)"
agent_sandbox_render="$(mktemp -t mandoforge-agent-sandbox-pilot-render.XXXXXX.out)"
cleanup() { rm -f "$k8s_render" "$default_render" "$pilot_render" "$agent_sandbox_render"; }
trap cleanup EXIT

kubectl kustomize deploy/k8s >"$k8s_render"
kubectl kustomize deploy >"$default_render"
kubectl kustomize deploy/remote-computer-pilot --load-restrictor LoadRestrictionsNone >"$pilot_render"
kubectl kustomize deploy/agent-sandbox-pilot --load-restrictor LoadRestrictionsNone >"$agent_sandbox_render"

if ! grep -q "claimName: mandoforge-remote-computer-state" "$k8s_render"; then
  echo "base Remote Computer render is missing the mounted state PVC claim" >&2
  exit 1
fi

if grep -Eq 'kind:[[:space:]]*Secret|replace-me|s3\.example\.com' "$default_render"; then
  echo "default deploy render must not include placeholder Remote Computer state Secrets" >&2
  exit 1
fi

if grep -q "mandoforge-api:8080" "$k8s_render" "$pilot_render" deploy/k8s/remote-computer-artifact-discovery-sidecar.yaml; then
  echo "Remote Computer artifact sidecar must target the mandoforge-api service port 8787, not 8080" >&2
  exit 1
fi

if ! grep -q "http://mandoforge-api:8787" "$k8s_render"; then
  echo "base Remote Computer render is missing the in-cluster API URL on port 8787" >&2
  exit 1
fi

if ! grep -q "driver: csi.juicefs.com" "$pilot_render"; then
  echo "Remote Computer pilot render is missing JuiceFS CSI storage" >&2
  exit 1
fi

if ! grep -q "volumeName: mandoforge-remote-computer-state-juicefs-pv" "$pilot_render"; then
  echo "Remote Computer pilot render does not bind the Pod-mounted state PVC to the JuiceFS PV" >&2
  exit 1
fi

if ! grep -q "kind: ScaledObject" "$pilot_render"; then
  echo "Remote Computer pilot render is missing the KEDA ScaledObject example" >&2
  exit 1
fi

if ! awk '
  /name: mandoforge-agent-remote-computer-warm-pool/ { in_warm_pool = 1 }
  in_warm_pool && /name: artifact-discovery/ { found_sidecar = 1 }
  in_warm_pool && /name: mandoforge-remote-computer-artifact-discovery/ { found_config = 1 }
  END { exit !(found_sidecar && found_config) }
' "$pilot_render"; then
  echo "Remote Computer warm-pool render is missing artifact discovery sidecar parity" >&2
  exit 1
fi

if grep -Eq 'kind:[[:space:]]*Secret|replace-me|s3\.example\.com' deploy/k8s/agent-sandbox-runtime.yaml; then
  echo "Agent Sandbox runtime manifest must not include placeholder Secrets" >&2
  exit 1
fi

for agent_sandbox_kind in SandboxTemplate SandboxWarmPool SandboxClaim; do
  if ! grep -q "kind: $agent_sandbox_kind" "$agent_sandbox_render"; then
    echo "Agent Sandbox pilot render is missing $agent_sandbox_kind" >&2
    exit 1
  fi
done

if ! grep -q "apiVersion: extensions.agents.x-k8s.io/v1beta1" "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot must use the current v1beta1 extensions API" >&2
  exit 1
fi

if ! grep -q "volumeClaimTemplates:" "$agent_sandbox_render" \
  || ! grep -q "name: workspace-data" "$agent_sandbox_render" \
  || ! grep -q "name: agent-state" "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot must define per-sandbox workspace and agent-state PVC templates" >&2
  exit 1
fi

if ! grep -q "claimName: mandoforge-agent-sandbox-project-cache" "$agent_sandbox_render" \
  || ! grep -q "CARGO_TARGET_DIR" "$agent_sandbox_render" \
  || ! grep -q "PNPM_STORE_DIR" "$agent_sandbox_render" \
  || ! grep -q "UV_CACHE_DIR" "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot must mount explicit dependency/build cache paths" >&2
  exit 1
fi

if ! grep -q "sandboxTemplateRef:" "$agent_sandbox_render" \
  || ! grep -q "warmPoolRef:" "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot must wire WarmPool and Claim with current reference fields" >&2
  exit 1
fi

if grep -q "templateRef:" "$agent_sandbox_render" \
  || grep -q "warmpool:" "$agent_sandbox_render"; then
  echo "Agent Sandbox pilot must not use retired SandboxClaim reference fields" >&2
  exit 1
fi

for tracking_key in \
  "mandoforge.io/session-id" \
  "mandoforge.io/remote-computer-id" \
  "mandoforge.io/tenant-id" \
  "mandoforge.io/lease-id" \
  "mandoforge.io/lifecycle"; do
  if ! grep -q "$tracking_key" "$runner_source"; then
    echo "Remote Computer runner is missing Kubernetes Pod tracking metadata: $tracking_key" >&2
    exit 1
  fi
done

if ! grep -q "parse_kubernetes_exec_command" "$runner_source" \
  || ! grep -q "metadata.command array must contain only non-empty string arguments" "$runner_source" \
  || ! grep -q "command_query" "$runner_source"; then
  echo "Remote Computer runner must preserve Kubernetes exec argv semantics and validate array commands" >&2
  exit 1
fi

if grep -q 'parts.join(" ")' "$runner_source"; then
  echo "Remote Computer runner must not collapse metadata.command arrays into shell strings" >&2
  exit 1
fi

echo "remote computer k8s manifests ok"
