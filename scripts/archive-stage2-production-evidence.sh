#!/usr/bin/env bash
set -euo pipefail

namespace="${MANDOFORGE_K8S_NAMESPACE:-agent-os}"
pvc_name="${MANDOFORGE_STAGE2_EVIDENCE_PVC:-mandoforge-stage2-production-evidence}"
pod_name="${MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_POD:-mandoforge-stage2-evidence-archive}"
archive_path="${1:-.mandoforge/stage2-production-evidence-$(date -u +%Y%m%dT%H%M%SZ).tar.gz}"
image="${MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_IMAGE:-busybox:1.36}"

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to archive Stage 2 production evidence" >&2
  exit 1
fi

mkdir -p "$(dirname "$archive_path")"

cleanup() {
  kubectl delete pod "$pod_name" --namespace "$namespace" --ignore-not-found=true >/dev/null 2>&1 || true
}

trap cleanup EXIT

cleanup

cat <<YAML | kubectl apply -f - >/dev/null
apiVersion: v1
kind: Pod
metadata:
  name: ${pod_name}
  namespace: ${namespace}
  labels:
    app.kubernetes.io/name: mandoforge
    app.kubernetes.io/component: stage2-production-evidence-archive
spec:
  restartPolicy: Never
  automountServiceAccountToken: false
  securityContext:
    seccompProfile:
      type: RuntimeDefault
  containers:
    - name: archive
      image: ${image}
      command:
        - sh
        - -c
        - sleep 3600
      securityContext:
        allowPrivilegeEscalation: false
        capabilities:
          drop:
            - ALL
        readOnlyRootFilesystem: true
        runAsNonRoot: true
        runAsUser: 1000
      resources:
        requests:
          cpu: 10m
          memory: 32Mi
        limits:
          cpu: 100m
          memory: 128Mi
      volumeMounts:
        - name: evidence
          mountPath: /evidence
          readOnly: true
  volumes:
    - name: evidence
      persistentVolumeClaim:
        claimName: ${pvc_name}
        readOnly: true
YAML

kubectl wait --for=condition=Ready "pod/${pod_name}" --namespace "$namespace" --timeout=120s >/dev/null
kubectl exec --namespace "$namespace" "$pod_name" -- tar czf - -C /evidence . >"$archive_path"

if [[ ! -s "$archive_path" ]]; then
  echo "Stage 2 production evidence archive is empty: $archive_path" >&2
  exit 1
fi

echo "Stage 2 production evidence archived to $archive_path" >&2
