# Deploy Instructions

- Treat Kubernetes and deployment manifests as production-facing control surfaces. Keep changes scoped to the manifest family you are modifying and preserve unrelated environment overlays.
- Do not include example secrets in default `kustomization.yaml` resources. Keep examples as standalone templates and use placeholders instead of default credentials.
- Production state must use durable storage. Do not use `emptyDir` for workspaces, evidence, database, queue, or Remote Computer state unless the manifest is explicitly test-only.
- Stage 2 evidence Jobs must mount the `mandoforge-stage2-production-evidence` claim and write to the contract paths checked by `scripts/verify-stage2-evidence-k8s-manifests.sh`.
- When changing deployment or evidence manifests, run:
  - `./scripts/verify-stage2-evidence-k8s-manifests.sh`
  - `STATIC_ONLY=1 ./scripts/production-launch-preflight.sh`

