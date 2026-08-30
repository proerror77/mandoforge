#!/usr/bin/env bash
set -euo pipefail

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to verify observability Kubernetes manifests" >&2
  exit 1
fi

manifest="deploy/k8s/otel-collector.yaml"
network_policy_manifest="deploy/k8s/otel-collector-networkpolicy.yaml"
worker_network_policy_manifests=(
  "deploy/k8s/worker-networkpolicy.yaml"
  "deploy/k8s/worker-isolated-pool-networkpolicy.yaml"
)
rendered="/tmp/mandoforge-observability-kustomize.out"

if [[ ! -f "$manifest" ]]; then
  echo "missing OTel collector manifest: $manifest" >&2
  exit 1
fi

if [[ ! -f "$network_policy_manifest" ]]; then
  echo "missing OTel collector NetworkPolicy manifest: $network_policy_manifest" >&2
  exit 1
fi

for worker_policy in "${worker_network_policy_manifests[@]}"; do
  if [[ ! -f "$worker_policy" ]]; then
    echo "missing worker NetworkPolicy manifest: $worker_policy" >&2
    exit 1
  fi
  for pattern in "app: mandoforge-otel-collector" "port: 4318" "port: 13133"; do
    if ! grep -q "$pattern" "$worker_policy"; then
      echo "$worker_policy is missing restricted OTel egress: $pattern" >&2
      exit 1
    fi
  done
  if grep -Eq 'cidr:[[:space:]]*(0\.0\.0\.0/0|::/0)' "$worker_policy"; then
    echo "$worker_policy must not allow unrestricted public egress" >&2
    exit 1
  fi
done

collector_identity="$(kubectl patch --local=true --type=merge --patch '{}' -f "$network_policy_manifest" \
  -o jsonpath='{.kind}|{.metadata.name}|{.spec.podSelector.matchLabels.app}')"
if [[ "$collector_identity" != "NetworkPolicy|mandoforge-otel-collector|mandoforge-otel-collector" ]]; then
  echo "OTel collector NetworkPolicy targets the wrong resource: $collector_identity" >&2
  exit 1
fi

collector_ingress_apps="$(kubectl patch --local=true --type=merge --patch '{}' -f "$network_policy_manifest" \
  -o jsonpath='{range .spec.ingress[*].from[*]}{.podSelector.matchLabels.app}{"\n"}{end}' | sort -u)"
if [[ "$collector_ingress_apps" != $'mandoforge-api\nmandoforge-worker\nmandoforge-worker-isolated' ]]; then
  echo "OTel collector NetworkPolicy has unexpected ingress sources" >&2
  exit 1
fi

collector_ingress_ports="$(kubectl patch --local=true --type=merge --patch '{}' -f "$network_policy_manifest" \
  -o jsonpath='{range .spec.ingress[*].ports[*]}{.protocol}:{.port}{"\n"}{end}' | sort -u)"
if [[ "$collector_ingress_ports" != $'TCP:13133\nTCP:4318' ]]; then
  echo "OTel collector NetworkPolicy has unexpected ingress ports" >&2
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
