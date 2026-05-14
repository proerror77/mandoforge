# Stage 1 Deployment And Demo Guide

This guide verifies the Stage 1 runtime loop:

```text
create session -> run harness -> policy approval -> approve -> execute tool -> artifact -> replay/audit
```

## Local Runtime

Start the API:

```bash
cargo run -p mandoforge-api
```

In another shell, run the full demo:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-demo.sh
```

Expected evidence:

- `event_types` includes `approval.requested`, `approval.approved`, `policy.requires_approval`, `tool.result`, and `artifact.created`.
- `tool_calls` includes completed `shell.exec` and completed `file.write`.
- `artifacts` includes `diagnostics.md`.
- `workspace_file` points to `.mandoforge/workspaces/<session_id>/diagnostics.md`.

To refresh generic demo facts in Postgres:

```bash
DATABASE_URL=postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge \
  COUNT=96 \
  ./scripts/seed-platform-events.sh
```

To verify the API is using Postgres for `sql.query`:

```bash
DATABASE_URL=postgres://mandoforge:mandoforge@127.0.0.1:5432/mandoforge \
BASE_URL=http://127.0.0.1:8787 \
./scripts/verify-postgres-sql-query.sh
```

This check fails if the API is accidentally running in in-memory fallback mode because the query result must contain rows from `generic_demo.platform_events`.

Open the static UI:

```text
http://127.0.0.1:8787
```

Use **Run Diagnostics Demo** to inspect the timeline, approval queue, artifact detail, and tool-call policy detail.

To use a real OpenAI-compatible provider instead of the deterministic mock:

```bash
MANDOFORGE_PROVIDER_BASE_URL=https://api.openai.com \
MANDOFORGE_PROVIDER_API_KEY=... \
MANDOFORGE_PROVIDER_MODEL=gpt-5.4-mini \
cargo run -p mandoforge-api
```

When either provider env var is absent, the runtime falls back to the mock provider so local tests and demos remain repeatable.

To run approved `shell.exec` calls through Docker instead of the host shell:

```bash
MANDOFORGE_SHELL_RUNNER=docker \
MANDOFORGE_SHELL_DOCKER_IMAGE=alpine:3.20 \
cargo run -p mandoforge-api
```

The Docker runner mounts only the session workspace at `/workspace`, disables container networking, and applies basic CPU/memory limits. `shell.exec` remains approval-gated in either runner mode.

After starting the API with Docker shell runner mode, verify it with:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/verify-docker-shell-runner.sh
```

## Docker Compose

Start the local stack:

```bash
docker compose up --build
```

Run the demo against the published port:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-demo.sh
```

Docker Compose uses Postgres through `DATABASE_URL`, so the demo validates migrations and durable runtime tables in addition to the API loop.

Run the full live gate after starting the API with Postgres and Docker shell runner mode:

```bash
RUN_LIVE=1 BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-final-gate.sh
```

Or let the gate start Compose Postgres and a host API process for the live checks:

```bash
RUN_LIVE=1 START_LIVE_STACK=1 ./scripts/stage1-final-gate.sh
```

The self-start mode keeps the API on the host so `MANDOFORGE_SHELL_RUNNER=docker` can call the host Docker CLI.

## Kubernetes Skeleton

The Kubernetes manifests are under `deploy/k8s`.

```bash
kubectl apply -k deploy/k8s
kubectl -n agent-os port-forward svc/mandoforge-api 8787:8787
BASE_URL=http://127.0.0.1:8787 ./scripts/stage1-demo.sh
```

Before shared-cluster use:

- Replace `deploy/k8s/secret.example.yaml`.
- Add NetworkPolicy before enabling shell, Codex, HTTP, or MCP workers.
- Replace `emptyDir` workspaces with durable artifact storage.
- Split sandbox and Codex execution into separate worker Deployments.

## Stage 2 Production Evidence

Stage 2 governed-runtime completion is gated by production evidence, not by local demos alone. After deploying the API and configuring the Stage 2 controller/provider targets, collect readiness evidence with:

```bash
ALLOW_BLOCKED=1 BASE_URL=http://127.0.0.1:8787 ./scripts/stage2-production-evidence-gate.sh
```

To execute the bounded controller-backed validation endpoints:

```bash
RUN_STAGE2_PRODUCTION_VALIDATIONS=1 \
MANDOFORGE_STAGE2_TEAM_ID=<team_uuid> \
BASE_URL=http://127.0.0.1:8787 \
./scripts/stage2-production-evidence-gate.sh
```

This gate remains fail-closed until `/api/stage2/readiness` reports no open completion gaps. See `docs/stage2-production-evidence-gate.md` for the optional flags that enable higher-impact KMS rotation, Remote Computer sidecar recovery, and finance close/reconciliation checks.

For in-cluster inventory runs, render or apply the opt-in Kubernetes Job:

```bash
kubectl kustomize deploy/stage2-evidence
```

The Job uses the published MandoForge image, calls `http://mandoforge-api:8787`, writes evidence to an ephemeral volume, and defaults to `ALLOW_BLOCKED=1` so it can inventory an incomplete Stage 2 deployment without pretending the stage is complete.

For strict production evidence, render the persistent evidence bundle:

```bash
kubectl kustomize deploy/stage2-production-evidence --load-restrictor LoadRestrictionsNone
```

That bundle includes the all-up Stage 2 production gate plus narrower evidence Jobs for observability collector, Remote Computer, provider governance, tenant isolation, Vault/KMS, approval notifications, worker operations, policy rollout orchestration, Codex App Server, and MCP Gateway. The observability Job runs `scripts/observability-collector-evidence-gate.sh`, calls the collector readiness plus deployment/cluster rollout validation endpoints, optionally runs remediation supervision, and writes evidence under the shared production evidence PVC for later archival. The Remote Computer Job runs `scripts/remote-computer-evidence-gate.sh`, calls the readiness/runner/state-sync validation endpoints, optionally records sidecar recovery evidence, and writes its output under the same PVC. The provider governance Job runs `scripts/provider-governance-evidence-gate.sh`, calls the provider summary/policy/deployment gates, optionally records provider rollout/rollback evidence, and writes its output under the same PVC. The tenant isolation Job runs `scripts/tenant-isolation-evidence-gate.sh`, calls readiness plus tenant routing validation, and writes its output under the same PVC. The Vault Job runs `scripts/vault-evidence-gate.sh`, calls readiness/health plus KMS recovery validation, optionally records KMS rotation evidence, and writes its output under the same PVC. The approval notification Job runs `scripts/approval-notification-evidence-gate.sh`, calls routing/run-history/deployment/ops validation endpoints, optionally records delivery evidence, and writes its output under the same PVC. The worker Job runs `scripts/worker-evidence-gate.sh`, calls worker readiness and load-validation endpoints, and writes its output under the same PVC. The policy rollout Job runs `scripts/policy-rollout-evidence-gate.sh`, calls orchestration readiness and validation endpoints, optionally records due-run evidence, and writes its output under the same PVC. The Codex App Server Job runs `scripts/codex-app-server-evidence-gate.sh`, calls health/control-plane/deployment/ops endpoints, optionally records stale-poll evidence, and writes its output under the same PVC. The MCP Gateway Job runs `scripts/mcp-gateway-evidence-gate.sh`, calls team-scoped rollout summary/history/deployment validation endpoints, optionally records due-run supervision evidence, and writes its output under the same PVC.
