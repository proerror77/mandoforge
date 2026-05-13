# Kubernetes Deployment Skeleton

This directory is the Stage 1 Kubernetes starting point for the Generic Agent OS Kernel.

It contains:

- Namespace.
- API Deployment and Service.
- Worker Deployment for queued execution jobs, with a restricted ServiceAccount, disabled token automount, RuntimeDefault seccomp, dropped capabilities, read-only root filesystem, resource bounds, and a worker NetworkPolicy.
- Worker HPA skeleton for CPU-based scaling experiments.
- Worker KEDA ScaledObject for queue-depth scaling experiments.
- Agent Remote Computer Pod template with zero replicas.
- Remote Computer service account, RWX state PVC placeholder, state-contract ConfigMap, and deny-by-default NetworkPolicy.
- JuiceFS CSI Remote Computer state example, kept outside the default kustomization.
- Remote Computer warm-pool example, kept outside the default kustomization.
- Remote Computer KEDA ScaledObject example, kept outside the default kustomization.
- Remote Computer pilot bundle at `../kustomization.yaml` that opts into JuiceFS, warm-pool, and Remote Computer KEDA examples together.
- Scheduler CronJob for due policy, approval, release, and MCP automation.
- Postgres StatefulSet and Service.
- ConfigMap for runtime configuration.
- Example Secret for local/dev credentials.
- Workspace `emptyDir` volume for Stage 1.

Apply locally after building and publishing an image:

```bash
kubectl apply -k deploy/k8s
kubectl -n agent-os port-forward svc/mandoforge-api 8787:8787
```

Render the opt-in Remote Computer pilot bundle before applying it to a real cluster:

```bash
kubectl kustomize deploy
```

Production notes:

- Replace the example Secret before deployment.
- Prefer external Postgres or a mature Postgres Operator for production.
- Replace `emptyDir` workspaces with PVC or object-storage-backed artifact sync before long-running workers.
- Review and adapt the worker NetworkPolicy before enabling shell, Codex, HTTP, or MCP execution in shared clusters.
- Keep Codex and sandbox execution disabled or tightly constrained before multi-tenant use; the current worker drains jobs through the API execution endpoint.
- Treat `worker-hpa.yaml` and `worker-keda.yaml` as autoscaling pilot manifests. KEDA is wired to a Prometheus queue-depth query, but you still need production metrics, load validation, and isolation policy before claiming production autoscaling.
- Treat the Remote Computer manifests as readiness skeletons until the runner gates are enabled. They include the Pod template, warm-pool example, queue scaler, state contract, and a JuiceFS state profile, but they do not by themselves prove production sidecar supervision or distributed Memory/Notes/Skills synchronization.
- Treat `remote-computer-state-contract.yaml` as the mounted state layout contract for `/agent-state/memory`, `/agent-state/notes`, `/agent-state/skills`, artifacts, locks, and manifests. Its conflict policy is one active writer per session; shared Memory/Notes/Skills must stay read-mostly until a lock-aware sync manager is configured.
- `remote-computer-artifact-discovery-sidecar.yaml` provides a fail-closed sidecar script for scanning `/workspace/artifacts` and pushing discovered files through `/api/remote-computers/artifacts/sync`. Keep `MANDOFORGE_ARTIFACT_DISCOVERY_ENABLED=false` until leased Pods receive real `MANDOFORGE_SESSION_ID` and `MANDOFORGE_REMOTE_COMPUTER_ID` values.
- Treat `remote-computer-state-juicefs-profile.yaml` as the opt-in JuiceFS CSI profile for the same `mandoforge-remote-computer-state` PVC mounted by Remote Computer Pods. Replace its secret values, metadata backend, object store, and namespace before applying it.
- Keep `remote-computer-state-juicefs-example.yaml` as a reference-only manifest for provider shape; it creates a separate sample PVC and is not the Pod-mounted production claim.
- Treat `remote-computer-warm-pool.yaml` as an opt-in example. It keeps placeholder Pods warm but does not yet lease, assign, or attach sessions to them.
- Treat `remote-computer-keda.yaml` as an opt-in example. It assumes Prometheus metrics that are not production-hardened yet.
- Treat `../kustomization.yaml` as the reviewable bundle for enabling those examples together; do not apply it until storage credentials, Prometheus metrics, namespace policy, and state conflict rules have been reviewed.
- Replace the scheduler's demo admin headers with a real service-account or gateway-auth path before production exposure.
