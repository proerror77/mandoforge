# Kubernetes Deployment Skeleton

This directory is the Stage 1 Kubernetes starting point for the Generic Agent OS Kernel.

It contains:

- Namespace.
- API Deployment and Service.
- Worker Deployment for queued execution jobs.
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
