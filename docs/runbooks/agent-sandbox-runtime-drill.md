# Agent Sandbox Runtime Drill

This runbook validates the MandoForge production-default Agent Sandbox
substrate without changing the source-of-truth boundary. MandoForge API/Postgres owns sessions, policy,
approvals, execution jobs, leases, events, artifacts, and audit logs. The
Kubernetes SIG Agent Sandbox controller owns `SandboxClaim`, `Sandbox`, Pod,
per-sandbox PVC, and warm-pool mechanics only.

The readiness endpoint remains `pilot_only` until a fresh successful summary is
present at `.mandoforge/agent-sandbox-runtime-evidence/summary.json` (or at
`MANDOFORGE_AGENT_SANDBOX_EVIDENCE_FILE`). The summary is ignored by Git and
must not contain credentials, prompts, file contents, or tenant data.

## Pinned Contract

- Agent Sandbox controller: `v0.5.1`.
- Kubernetes API: `agents.x-k8s.io/v1beta1` and
  `extensions.agents.x-k8s.io/v1beta1`.
- Runtime image: `ghcr.io/proerror77/mandoforge/mandoforge-agent-sandbox-runtime@sha256:57b4477d7b9238859c81a5a2ba799acce69907743c12d622fb79e5a8c30badcc`,
  published from the Git index by `scripts/build-agent-sandbox-runtime-image.sh`.
- Environment profile:

```json
{
  "profile": "agent-sandbox",
  "namespace": "agent-os",
  "sandbox_warm_pool": "mandoforge-agent-runtime",
  "cache_scope": "mandoforge",
  "workspace_seed": "mandoforge"
}
```

`workspace_seed` identifies the source snapshot seeded into a new workspace.
`cache_scope` identifies the project-level dependency cache. Neither field is a
secret. The runner records both on the Claim and in MandoForge DB metadata; it
does not copy custom `mandoforge.io/*` metadata into
`spec.additionalPodMetadata`, because the controller validates propagated Pod
metadata against an allowlist.

## Preconditions

1. Use a disposable or explicitly approved cluster context. The local drill
   below requires `docker-desktop`:

```bash
test "$(kubectl config current-context)" = "${EXPECTED_KUBE_CONTEXT:-docker-desktop}"
kubectl version
```

2. Install the pinned upstream controller bundle, then verify all four CRDs use
   `v1beta1` and the controller is Ready:

```bash
curl -fsSLo /tmp/agent-sandbox-v0.5.1-manifest.yaml https://github.com/kubernetes-sigs/agent-sandbox/releases/download/v0.5.1/manifest.yaml
curl -fsSLo /tmp/agent-sandbox-v0.5.1-extensions.yaml https://github.com/kubernetes-sigs/agent-sandbox/releases/download/v0.5.1/extensions.yaml
printf '%s  %s\n' 8cfdf0a878f66b91d2e7103e77859d1412d850ce3f5fe5c3fa134c36bd55504a /tmp/agent-sandbox-v0.5.1-manifest.yaml | shasum -a 256 -c -
printf '%s  %s\n' 7c22b450e24ede3fddbcd5ae0ee7c78ea102d6c30635ff860cc486578a55932e /tmp/agent-sandbox-v0.5.1-extensions.yaml | shasum -a 256 -c -
kubectl apply -f /tmp/agent-sandbox-v0.5.1-manifest.yaml
kubectl apply -f /tmp/agent-sandbox-v0.5.1-extensions.yaml
kubectl -n agent-sandbox-system rollout status deployment/agent-sandbox-controller
kubectl get crd sandboxes.agents.x-k8s.io \
  sandboxclaims.extensions.agents.x-k8s.io \
  sandboxtemplates.extensions.agents.x-k8s.io \
  sandboxwarmpools.extensions.agents.x-k8s.io
```

3. Build the dedicated runtime and verify the repository contracts:

```bash
scripts/build-agent-sandbox-runtime-image.sh
scripts/verify-remote-computer-k8s-manifests.sh
kubectl apply -f deploy/k8s/namespace.yaml
# Disposable local drill only; production uses controlled secret delivery.
kubectl apply -n agent-os -f deploy/k8s/secret.example.yaml
kubectl apply --server-side --dry-run=server -k deploy/k8s
```

4. Apply the default bundle. It contains a dedicated API identity and scoped
   Agent Sandbox RBAC; queue workers remain uncredentialed:

```bash
kubectl apply -k deploy/k8s
```

The runtime Pod service account is `mandoforge-remote-computer` with token
automount disabled. The runner executes in the API process through
`mandoforge-api`, whose Role permits SandboxClaim lifecycle, Sandbox/Pod lookup,
and `pods/exec`, but not direct Pod create/delete. Queue workers call the API and
do not mount Kubernetes credentials.

### Docker Desktop Storage Override

The tracked cache PVC requests `ReadWriteMany` and 50 GiB for a production
shared cache provider. Docker Desktop's default local-path provisioner supports
`ReadWriteOnce`, so the local drill may replace only that live PVC with an RWO
5 GiB claim labeled:

```yaml
mandoforge.io/local-drill-override: docker-desktop-rwo
```

Do not commit that override and do not use it as production RWX evidence.

## Low-Level Lifecycle Checks

Apply `deploy/agent-sandbox-smoke` and record Claim creation through Ready:

```bash
started_at="$(date +%s)"
kubectl apply -k deploy/agent-sandbox-smoke
kubectl -n agent-os wait --for=condition=Ready \
  sandboxclaim/mandoforge-agent-runtime-smoke --timeout=120s
ready_at="$(date +%s)"
echo "claim_ready_seconds=$((ready_at-started_at))"
```

Resolve the generated names from status and the Sandbox annotation, never by
assuming Claim, Sandbox, and Pod names are equal:

```bash
sandbox="$(kubectl -n agent-os get sandboxclaim mandoforge-agent-runtime-smoke \
  -o jsonpath='{.status.sandbox.name}')"
pod="$(kubectl -n agent-os get sandbox "$sandbox" \
  -o jsonpath='{.metadata.annotations.agents\.x-k8s\.io/pod-name}')"
kubectl -n agent-os get \
  sandboxclaim/mandoforge-agent-runtime-smoke \
  "sandbox/$sandbox" "pod/$pod"
```

Inside the Pod, verify UID/GID 1000, read-only root filesystem, runtime
versions, and the fixed launcher. Run two governed operations with one session
UUID and confirm a marker remains. Start a different session through the
MandoForge allocation path and confirm it binds a different Claim/Sandbox and
PVC; in that second Pod, confirm the first Sandbox workspace is not present
while `/cache/project` retains a non-sensitive dependency-cache marker.

The workspace PVC is sandbox-private. Only dependency/tool caches under
`/cache/project` are shared by the declared project scope. Do not place target
outputs, prompts, credentials, home directories, or CLI conversation state in
the shared cache. Verify that Cargo `registry` and `git` resolve into the shared
cache while `.cargo-home/credentials.toml`, `.home`, and `target` remain under
the session workspace and are absent from a second Sandbox.

## Network Checks

The tracked policies must prove all of the following from the runtime Pod:

- DNS to cluster DNS works.
- MandoForge API ingress is reachable only through Pods carrying the expected
  API labels and port.
- An unlabeled private Pod is unreachable.
- `169.254.0.0/16`, including cloud metadata addresses, is unreachable.
- External TCP 443 is reachable when the pilot needs package/provider access.
- External TCP 80 is blocked.
- The runtime Pod has no service-account token mounted.

Standard Kubernetes `NetworkPolicy` is IP/port based, not FQDN aware. The runtime
therefore allows external 443 broadly; use a Cilium FQDN policy, egress proxy,
or equivalent for domain allowlists. Standard policy implementations can also
exempt traffic to the node or host-network API endpoint. Keep the runtime Pod
uncredentialed and use a host firewall/Cilium host policy when a network-level
API-server block is required.

## MandoForge Approval And Exec Check

Run MandoForge against Postgres with `MANDOFORGE_EXECUTION_WORKER=queue` and the
full explicit execution gates. For an out-of-cluster API process, use a
short-lived `mandoforge-api` ServiceAccount token and the cluster CA:

```bash
kubectl config view --raw --minify \
  -o jsonpath='{.clusters[0].cluster.certificate-authority-data}' \
  | base64 --decode > /tmp/mandoforge-kubernetes-ca.crt
kubectl -n agent-os create token mandoforge-api --duration=1h \
  > /tmp/mandoforge-kubernetes-api-token
chmod 600 \
  /tmp/mandoforge-kubernetes-ca.crt \
  /tmp/mandoforge-kubernetes-api-token

export MANDOFORGE_REMOTE_COMPUTER_RUNNER=agent-sandbox
export MANDOFORGE_REMOTE_COMPUTER_NAMESPACE=agent-os
export MANDOFORGE_REMOTE_COMPUTER_KUBE_API_URL="$(kubectl config view --raw --minify -o jsonpath='{.clusters[0].cluster.server}')"
export MANDOFORGE_REMOTE_COMPUTER_BEARER_TOKEN_PATH=/tmp/mandoforge-kubernetes-api-token
export MANDOFORGE_REMOTE_COMPUTER_CA_CERT_PATH=/tmp/mandoforge-kubernetes-ca.crt
export MANDOFORGE_REMOTE_COMPUTER_MUTATION_ENABLED=true
export MANDOFORGE_REMOTE_COMPUTER_LIVE_MUTATION_ENABLED=true
export MANDOFORGE_REMOTE_COMPUTER_EXECUTION_ENABLED=true
export MANDOFORGE_REMOTE_COMPUTER_EXECUTION_TRANSPORT=kubernetes
```

Create an active `remote_computer` Environment with the profile above, create a
session, and request `file.write`. Before approval, assert there is no new Claim
and no execution job. Approve through `/api/approvals/{id}/approve`, drain the
resulting job as a worker principal, and assert:

- Job and tool call are `completed`.
- One RemoteComputer and active lease point to the resolved Pod.
- The file exists under `/workspace/sessions/<session-id>/...`.
- The event stream contains `remote_computer.on_demand_pod_provisioned`, handoff
  assigned/acknowledged, transport completed, `tool.result`, `artifact.created`,
  and `execution.completed`.
- Audit contains approval, provisioning, handoff, transport, and tool completion.
- A second approved job in the same session creates no Claim and reuses the same
  lease and workspace.

Send a `user.interrupt` session event after the checks. The session must become
terminal, the on-demand Claim/Sandbox/Pod/PVC must disappear, the lease must no
longer be active, and the RemoteComputer must not remain reusable. Reapplying
the same Claim before cleanup must retain one object and one UID. Also run a
short `restartPolicy: Never` template with `ttlSecondsAfterFinished` and prove
the controller removes the finished Claim and its runtime.

## Evidence Summary

Write a tenant-safe summary with this required shape:

```json
{
  "schema_version": 1,
  "status": "passed",
  "captured_at": "2026-07-10T07:03:25Z",
  "cluster_context": "docker-desktop",
  "controller_version": "v0.5.1",
  "validation_scope": "local_pilot",
  "checks": {
    "controller_ready": true,
    "claim_bound": true,
    "pod_ready": true,
    "runtime_versions": true,
    "workspace_reuse": true,
    "cross_session_isolation": true,
    "cache_scope": true,
    "network_policy": true,
    "cancel_cleanup": true,
    "ttl_cleanup": true,
    "retry_idempotency": true,
    "approved_exec": true,
    "durable_event": true,
    "artifact": true,
    "audit_log": true
  }
}
```

The readiness API rejects missing checks, false checks, malformed timestamps,
evidence older than `MANDOFORGE_AGENT_SANDBOX_EVIDENCE_MAX_AGE_HOURS` (168 by
default), and controller versions different from
`MANDOFORGE_AGENT_SANDBOX_CONTROLLER_VERSION` (`v0.5.1` by default).
`validation_scope=local_pilot` keeps overall readiness `pilot_only` even when
all live checks pass. Only a rerun on the intended production cluster with
`validation_scope=production_target` can clear the Agent Sandbox production
blocker. Production-target evidence must also include all-true
`production_checks` for `target_cluster`, `rwx_cache`,
`distributed_state_sync`, `network_enforcement`, `load_validation`, and
`rollback_validation`; changing the scope string alone does not promote the
runtime.

## Rollback

1. Set the three mutation/execution gates to `false` and restart the API and workers.
2. Send terminal session events or delete only Claims labeled
   `mandoforge.io/runtime-substrate=agent-sandbox` in the validated namespace.
3. Wait for generated Sandboxes, Pods, and per-sandbox PVCs to disappear.
4. Delete the smoke overlay. Delete `deploy/k8s` only when the namespace is a disposable drill environment; it is now the default application bundle.
5. Remove the short-lived API token and CA files.
6. Keep MandoForge DB events and audit logs; they are the durable history.

Do not delete the controller or CRDs until no other namespace uses them.
