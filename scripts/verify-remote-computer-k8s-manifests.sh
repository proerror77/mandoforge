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
)

for manifest in "${dry_run_manifests[@]}"; do
  if [[ ! -f "$manifest" ]]; then
    echo "missing Remote Computer manifest: $manifest" >&2
    exit 1
  fi
done

kubectl kustomize deploy/k8s >/tmp/mandoforge-k8s-render.out
kubectl kustomize deploy >/tmp/mandoforge-remote-computer-pilot-render.out

if ! grep -q "claimName: mandoforge-remote-computer-state" /tmp/mandoforge-k8s-render.out; then
  echo "base Remote Computer render is missing the mounted state PVC claim" >&2
  exit 1
fi

if grep -q "mandoforge-api:8080" /tmp/mandoforge-k8s-render.out /tmp/mandoforge-remote-computer-pilot-render.out deploy/k8s/remote-computer-artifact-discovery-sidecar.yaml; then
  echo "Remote Computer artifact sidecar must target the mandoforge-api service port 8787, not 8080" >&2
  exit 1
fi

if ! grep -q "http://mandoforge-api:8787" /tmp/mandoforge-k8s-render.out; then
  echo "base Remote Computer render is missing the in-cluster API URL on port 8787" >&2
  exit 1
fi

if ! grep -q "driver: csi.juicefs.com" /tmp/mandoforge-remote-computer-pilot-render.out; then
  echo "Remote Computer pilot render is missing JuiceFS CSI storage" >&2
  exit 1
fi

if ! grep -q "volumeName: mandoforge-remote-computer-state-juicefs-pv" /tmp/mandoforge-remote-computer-pilot-render.out; then
  echo "Remote Computer pilot render does not bind the Pod-mounted state PVC to the JuiceFS PV" >&2
  exit 1
fi

if ! grep -q "kind: ScaledObject" /tmp/mandoforge-remote-computer-pilot-render.out; then
  echo "Remote Computer pilot render is missing the KEDA ScaledObject example" >&2
  exit 1
fi

if ! awk '
  /name: mandoforge-agent-remote-computer-warm-pool/ { in_warm_pool = 1 }
  in_warm_pool && /name: artifact-discovery/ { found_sidecar = 1 }
  in_warm_pool && /name: mandoforge-remote-computer-artifact-discovery/ { found_config = 1 }
  END { exit !(found_sidecar && found_config) }
' /tmp/mandoforge-remote-computer-pilot-render.out; then
  echo "Remote Computer warm-pool render is missing artifact discovery sidecar parity" >&2
  exit 1
fi

echo "remote computer k8s manifests ok"
