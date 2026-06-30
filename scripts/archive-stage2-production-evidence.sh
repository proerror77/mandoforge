#!/usr/bin/env bash
set -euo pipefail

namespace="${MANDOFORGE_K8S_NAMESPACE:-agent-os}"
pvc_name="${MANDOFORGE_STAGE2_EVIDENCE_PVC:-mandoforge-stage2-production-evidence}"
pod_name="${MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_POD:-mandoforge-stage2-evidence-archive}"
archive_path="${1:-.mandoforge/stage2-production-evidence-$(date -u +%Y%m%dT%H%M%SZ).tar.gz}"
image="${MANDOFORGE_STAGE2_EVIDENCE_ARCHIVE_IMAGE:-busybox:1.36}"
VERIFY_ARCHIVE="${VERIFY_STAGE2_EVIDENCE_ARCHIVE:-1}"
ALLOW_UNVERIFIED_ARCHIVE="${ALLOW_UNVERIFIED_STAGE2_EVIDENCE_ARCHIVE:-0}"

if ! command -v kubectl >/dev/null 2>&1; then
  echo "kubectl is required to archive Stage 2 production evidence" >&2
  exit 1
fi

sha256_file="${archive_path}.sha256"
manifest_file="${archive_path}.manifest.txt"
created_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
current_context="$(kubectl config current-context 2>/dev/null || true)"
verifier="scripts/verify-stage2-evidence-archive.sh"

archive_size() {
  if stat -c%s "$archive_path" >/dev/null 2>&1; then
    stat -c%s "$archive_path"
  else
    stat -f%z "$archive_path"
  fi
}

archive_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$archive_path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$archive_path" | awk '{print $1}'
  else
    echo "sha256sum or shasum is required to checksum the Stage 2 evidence archive" >&2
    exit 1
  fi
}

write_manifest() {
  local verification_status="${1:?verification status is required}"
  local customer_grade_evidence="${2:?customer-grade evidence flag is required}"

  {
    echo "created_at=$created_at"
    echo "kube_context=${current_context:-<unknown>}"
    echo "namespace=$namespace"
    echo "pvc=$pvc_name"
    echo "archive_pod=$pod_name"
    echo "archive_image=$image"
    echo "archive_path=$archive_path"
    echo "archive_bytes=$archive_bytes"
    echo "archive_sha256=$archive_sha"
    echo "sha256_file=$sha256_file"
    echo "verification_required=true"
    echo "verification_status=$verification_status"
    echo "verifier=$verifier"
    echo "verify_stage2_evidence_archive=$VERIFY_ARCHIVE"
    echo "break_glass_unverified=$ALLOW_UNVERIFIED_ARCHIVE"
    echo "customer_grade_evidence=$customer_grade_evidence"
  } >"$manifest_file"
}

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

archive_sha="$(archive_sha256)"
archive_bytes="$(archive_size)"

printf '%s  %s\n' "$archive_sha" "$archive_path" >"$sha256_file"
write_manifest "pending" "false"

echo "Stage 2 production evidence archived to $archive_path" >&2
echo "Stage 2 production evidence checksum written to $sha256_file" >&2
echo "Stage 2 production evidence manifest written to $manifest_file" >&2

if [[ "$VERIFY_ARCHIVE" != "1" && "$ALLOW_UNVERIFIED_ARCHIVE" != "1" ]]; then
  write_manifest "blocked_verification_disabled" "false"
  echo "Stage 2 production evidence archive verification is mandatory; set ALLOW_UNVERIFIED_STAGE2_EVIDENCE_ARCHIVE=1 only for break-glass diagnostics." >&2
  exit 1
fi

if [[ ! -x "$verifier" ]]; then
  write_manifest "blocked_missing_verifier" "false"
  echo "Stage 2 evidence archive verifier is missing or not executable: $verifier" >&2
  exit 1
fi

if [[ "$VERIFY_ARCHIVE" == "1" ]]; then
  if ALLOW_PENDING_STAGE2_ARCHIVE_MANIFEST=1 "$verifier" "$archive_path" >&2; then
    write_manifest "passed" "true"
    echo "Stage 2 production evidence manifest verified: $manifest_file" >&2
  else
    write_manifest "failed" "false"
    echo "Stage 2 production evidence archive verification failed; manifest records verification_status=failed: $manifest_file" >&2
    exit 1
  fi
else
  write_manifest "skipped_break_glass" "false"
  echo "Stage 2 production evidence archive verification skipped by break-glass override; this archive is not customer-grade evidence." >&2
fi
