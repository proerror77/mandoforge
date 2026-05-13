#!/usr/bin/env bash
set -euo pipefail

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to verify observability Kubernetes manifests" >&2
  exit 1
fi

manifest="deploy/k8s/otel-collector.yaml"
network_policy_manifest="deploy/k8s/otel-collector-networkpolicy.yaml"
rendered="/tmp/mandoforge-observability-kustomize.out"

if [[ ! -f "$manifest" ]]; then
  echo "missing OTel collector manifest: $manifest" >&2
  exit 1
fi

if [[ ! -f "$network_policy_manifest" ]]; then
  echo "missing OTel collector NetworkPolicy manifest: $network_policy_manifest" >&2
  exit 1
fi

kubectl kustomize deploy/k8s >"$rendered"

required_patterns=(
  "name: mandoforge-otel-collector"
  "name: mandoforge-otel-collector-config"
  "containerPort: 4318"
  "containerPort: 13133"
  "MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT: http://mandoforge-otel-collector:4318"
  "MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT: http://mandoforge-otel-collector:13133/healthz"
  "kind: NetworkPolicy"
  "readOnlyRootFilesystem: true"
  "automountServiceAccountToken: false"
  "app: mandoforge-api"
)

for pattern in "${required_patterns[@]}"; do
  if ! grep -q "$pattern" "$rendered"; then
    echo "observability Kubernetes render is missing: $pattern" >&2
    exit 1
  fi
done

echo "observability k8s manifests ok"
