# Kubernetes Deployment Skeleton

This directory is the Stage 1 Kubernetes starting point for the Generic Agent OS Kernel.

It contains:

- Namespace.
- API Deployment and Service.
- OTel Collector Deployment, Service, and NetworkPolicy for in-cluster OTLP logs/traces/metrics ingestion, with a separate health endpoint used by the API readiness gate.
- Worker Deployment for queued execution jobs, with a restricted ServiceAccount, disabled token automount, RuntimeDefault seccomp, dropped capabilities, read-only root filesystem, resource bounds, and a worker NetworkPolicy.
- Worker HPA skeleton for CPU-based scaling experiments.
- Worker KEDA ScaledObject for queue-depth scaling experiments.
- Agent Sandbox controller contract pinned to `v0.5.1`, plus the default `SandboxTemplate`, `SandboxWarmPool`, per-sandbox workspace PVC, project dependency cache, and ingress/egress NetworkPolicy.
- A dedicated API ServiceAccount and namespace-scoped RBAC for SandboxClaim lifecycle, Sandbox/Pod discovery, and `pods/exec`. Queue workers do not receive Kubernetes API credentials.
- Remote Computer service account, RWX state PVC placeholder, and state-contract ConfigMap. The legacy direct-Pod template remains a reference and is not in the default kustomization.
- JuiceFS CSI Remote Computer state example, kept outside the default kustomization.
- Remote Computer warm-pool example, kept outside the default kustomization.
- Remote Computer KEDA ScaledObject example, kept outside the default kustomization.
- Remote Computer pilot bundle at `../remote-computer-pilot/kustomization.yaml` that opts into JuiceFS, warm-pool, and Remote Computer KEDA examples together.
- Agent Sandbox compatibility bundle at `../agent-sandbox-pilot/kustomization.yaml`; it now renders the same production-default substrate as `deploy/k8s` and creates no live claim.
- Agent Sandbox smoke bundle at `../agent-sandbox-smoke/kustomization.yaml` that adds a live `SandboxClaim` example for explicit controller smoke tests.
- Scheduler CronJob for due policy, approval, release, and MCP automation, using a dedicated ServiceAccount with token automount disabled and Secret-sourced scheduler subject, role, and shared token headers.
- Postgres StatefulSet and Service.
- ConfigMap for runtime configuration.
- Example Secret template for local/dev credentials. It is intentionally not part of the default kustomization; create `mandoforge-secrets` out of band before starting Pods.
- Secret delivery contract ConfigMap documenting that production must supply `mandoforge-secrets` through an external secret manager, External Secrets Operator, SealedSecret, or equivalent controlled path.
- Durable workspace PVC for API-owned workspaces.

Install the pinned Agent Sandbox controller, create a local/dev Secret, then
apply locally after building and publishing the runtime and API images:

```bash
curl -fsSLo /tmp/agent-sandbox-manifest.yaml https://github.com/kubernetes-sigs/agent-sandbox/releases/download/v0.5.1/manifest.yaml
curl -fsSLo /tmp/agent-sandbox-extensions.yaml https://github.com/kubernetes-sigs/agent-sandbox/releases/download/v0.5.1/extensions.yaml
printf '%s  %s\n' 8cfdf0a878f66b91d2e7103e77859d1412d850ce3f5fe5c3fa134c36bd55504a /tmp/agent-sandbox-manifest.yaml | shasum -a 256 -c -
printf '%s  %s\n' 7c22b450e24ede3fddbcd5ae0ee7c78ea102d6c30635ff860cc486578a55932e /tmp/agent-sandbox-extensions.yaml | shasum -a 256 -c -
kubectl apply -f /tmp/agent-sandbox-manifest.yaml
kubectl apply -f /tmp/agent-sandbox-extensions.yaml
kubectl apply -f deploy/k8s/namespace.yaml
kubectl apply -n agent-os -f deploy/k8s/secret.example.yaml
kubectl apply -k deploy/k8s
kubectl -n agent-os port-forward svc/mandoforge-api 8787:8787
```

Render the opt-in Remote Computer pilot bundle before applying it to a real cluster:

```bash
kubectl kustomize deploy/remote-computer-pilot --load-restrictor LoadRestrictionsNone
```

The historical Agent Sandbox pilot path is now a compatibility alias for the
default bundle:

```bash
kubectl kustomize deploy/agent-sandbox-pilot --load-restrictor LoadRestrictionsNone
```

Render the Agent Sandbox smoke bundle only when you intentionally want to allocate a live sandbox claim:

```bash
kubectl kustomize deploy/agent-sandbox-smoke --load-restrictor LoadRestrictionsNone
```

Production notes:

- Do not apply `secret.example.yaml` to production. The default manifests include only the secret delivery contract; create `mandoforge-secrets` from a secret manager, External Secrets Operator, SealedSecret, or equivalent reviewed delivery path.
- Prefer external Postgres or a mature Postgres Operator for production.
- Review the workspace PVC storage class, backup policy, and retention policy before long-running workers.
- Review and adapt the worker NetworkPolicy before enabling shell, Codex, HTTP, or MCP execution in shared clusters.
- Keep Codex and sandbox execution disabled or tightly constrained before multi-tenant use; the worker drains the durable Postgres queue directly and does not call API `/run` endpoints.
- The checked-in production configuration selects `agent-sandbox` for environment lifecycle but keeps execution, mutation, and live mutation disabled. Until a separate narrow Kubernetes bridge exists, keep all three gates disabled; a queue worker configured for Kubernetes live execution fails startup and must not receive Kubernetes API credentials or RBAC.
- Treat `worker-hpa.yaml` and `worker-keda.yaml` as queue-worker autoscaling manifests. KEDA scales job claimers from queue pressure; it does not decide Workflow authority, approvals, policy, or Sandbox lifecycle. Production metrics and load validation are still required.
- Treat the Remote Computer manifests as readiness skeletons until the runner gates are enabled. They include the Pod template, warm-pool example, queue scaler, state contract, and a JuiceFS state profile, but they do not by themselves prove production sidecar supervision or distributed Memory/Notes/Skills synchronization.
- Treat `remote-computer-state-contract.yaml` as the mounted state layout contract for `/agent-state/memory`, `/agent-state/notes`, `/agent-state/skills`, artifacts, locks, and manifests. Its conflict policy is one active writer per session; shared Memory/Notes/Skills must stay read-mostly until a lock-aware sync manager is configured.
- `remote-computer-artifact-discovery-sidecar.yaml` provides a fail-closed sidecar script for scanning the assigned workspace artifact directory and pushing discovered files through `/api/remote-computers/artifacts/sync`. Keep `MANDOFORGE_ARTIFACT_DISCOVERY_ENABLED=false` until leased Pods receive real `MANDOFORGE_SESSION_ID`, `MANDOFORGE_REMOTE_COMPUTER_ID`, and assignment-aware artifact paths.
- `POST /api/remote-computers/sidecars/recovery/run` produces an audited replacement plan for missing or stale sidecar heartbeats. It only attempts Pod delete/create when the Kubernetes runner live mutation gates and `MANDOFORGE_REMOTE_COMPUTER_SIDECAR_REPLACEMENT_ENABLED=true` are all set.
- Treat `remote-computer-state-juicefs-profile.yaml` plus `remote-computer-state-juicefs-pvc-patch.yaml` as the opt-in JuiceFS CSI profile for the same `mandoforge-remote-computer-state` PVC mounted by Remote Computer Pods. Replace its secret values, metadata backend, object store, and namespace before applying it through the `../remote-computer-pilot/kustomization.yaml` pilot bundle.
- Keep `remote-computer-state-juicefs-example.yaml` as a reference-only manifest for provider shape; it creates a separate sample PVC and is not the Pod-mounted production claim.
- Treat `remote-computer-warm-pool.yaml` as an opt-in example. The worker can claim persisted warm-pool Remote Computer records, but a production controller still needs to register prewarmed Pods, reset dirty workspaces, and refill the pool.
- Treat `remote-computer-keda.yaml` as an opt-in example. It assumes Prometheus metrics that are not production-hardened yet.
- Treat `../remote-computer-pilot/kustomization.yaml` as the reviewable bundle for enabling those examples together; do not apply it until storage credentials, Prometheus metrics, namespace policy, and state conflict rules have been reviewed.
- The default bundle requires the pinned upstream Agent Sandbox CRDs/controller and intentionally creates no live `SandboxClaim`; render `../agent-sandbox-smoke/kustomization.yaml` only for an explicit allocation smoke test.
- The Agent Sandbox template uses a per-sandbox PVC for `/workspace/sessions`, plus a single-project cache PVC for Cargo registry/git downloads, pnpm, uv, and sccache. The launcher keeps Cargo credentials, `HOME`, and `CARGO_TARGET_DIR` inside the session workspace. Create separate cache PVCs per repository, tenant, or worker pool before broad shared-cluster use; never put prompts, credentials, CLI conversation state, or target outputs in the shared cache.
- MandoForge remains authoritative for WorkflowRun, TaskGrant, policy, approval, context, events, artifacts, and audit. The API creates a `SandboxClaim`, Agent Sandbox allocates the Sandbox/Pod/PVC, and the API resolves `agents.x-k8s.io/pod-name` before using Kubernetes `pods/exec`. KEDA only scales queue workers.
- Replace the scheduler example shared token before production exposure. If `MANDOFORGE_SCHEDULER_TOKEN` is set in the API runtime, `/api/scheduler/run-due` requires the CronJob to send the matching `x-mandoforge-scheduler-token` header in addition to Admin authorization.
- The bundled OTel Collector exports to the collector `debug` exporter so local clusters have a real OTLP target without external credentials. Its NetworkPolicy allows ingress from the API Pod to OTLP HTTP and health ports only. Replace or extend its exporter pipeline before claiming production collector rollout; keep `MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT` pointed at the collector health extension rather than the OTLP HTTP receiver.
