# Kubernetes Deployment Skeleton

This directory is the Stage 1 Kubernetes starting point for the Generic Agent OS Kernel.

It contains:

- Namespace.
- API Deployment and Service.
- Worker Deployment for queued execution jobs.
- Worker HPA skeleton for CPU-based scaling experiments.
- Worker KEDA ScaledObject for queue-depth scaling experiments.
- Agent Remote Computer Pod template with zero replicas.
- Remote Computer service account, RWX state PVC placeholder, and deny-by-default NetworkPolicy.
- JuiceFS CSI Remote Computer state example, kept outside the default kustomization.
- Remote Computer warm-pool example, kept outside the default kustomization.
- Remote Computer KEDA ScaledObject example, kept outside the default kustomization.
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

Production notes:

- Replace the example Secret before deployment.
- Prefer external Postgres or a mature Postgres Operator for production.
- Replace `emptyDir` workspaces with PVC or object-storage-backed artifact sync before long-running workers.
- Add NetworkPolicy before enabling shell, Codex, HTTP, or MCP execution in shared clusters.
- Keep Codex and sandbox execution disabled or tightly constrained before multi-tenant use; the current worker drains jobs through the API execution endpoint.
- Treat `worker-hpa.yaml` and `worker-keda.yaml` as autoscaling pilot manifests. KEDA is wired to a Prometheus queue-depth query, but you still need production metrics, load validation, and isolation policy before claiming production autoscaling.
- Treat the Remote Computer manifests as readiness skeletons only. They do not yet create per-session Pod leases, warm pools, or distributed Memory/Notes/Skills synchronization.
- Treat `remote-computer-state-juicefs-example.yaml` as an opt-in example. Replace its secret values, namespace, object store, and metadata backend before applying it.
- Treat `remote-computer-warm-pool.yaml` as an opt-in example. It keeps placeholder Pods warm but does not yet lease, assign, or attach sessions to them.
- Treat `remote-computer-keda.yaml` as an opt-in example. It assumes Prometheus metrics that are not production-hardened yet.
- Replace the scheduler's demo admin headers with a real service-account or gateway-auth path before production exposure.
