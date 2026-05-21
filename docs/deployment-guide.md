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

To rehearse the controller wiring locally before pointing at real controller systems:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/stage2-controller-drill.sh
```

This starts a local mock controller, sets the Stage 2 controller URL environment variables for the current process, runs the production evidence gate with validation coverage enabled, and writes evidence under `.mandoforge/stage2-controller-drill-evidence/`. It is a wiring drill, not production completion proof.

For a self-contained local rehearsal that starts an ephemeral API process first:

```bash
./scripts/stage2-controller-drill-live-gate.sh
```

The live gate uses the in-memory API store by default, injects mock controller URLs into the temporary API, runs the same mock-controller evidence path with optional controller actions enabled, and writes `.mandoforge/stage2-controller-drill-live-evidence/`. It is useful for CI and local wiring checks when you do not already have a running API.

## Whiskey Production-Like Pilot

`wishky-2-1` can run a loopback-only production-like adoption pilot without exposing the MandoForge API to the public internet:

```bash
MANDOFORGE_IMAGE_TAG=<tag> scripts/whiskey-adoption-deploy.sh
scripts/whiskey-adoption-evidence.sh
```

The API listens on `127.0.0.1:18787`, evidence is written under `/opt/mandoforge-adoption/evidence`, archives are written under `/opt/mandoforge-adoption/archives`, and local copies sync to `.mandoforge/remote-adoption/whiskey/`.

See [Whiskey Adoption Runbook](whiskey-adoption-runbook.md). This is an inventory/adoption lane, not a production completion claim unless real controller targets and credentials are configured.

For in-cluster inventory runs, render or apply the opt-in Kubernetes Job:

```bash
kubectl kustomize deploy/stage2-evidence
```

The Job uses the published MandoForge image, calls `http://mandoforge-api:8787`, writes evidence to an ephemeral volume, and defaults to `ALLOW_BLOCKED=1` so it can inventory an incomplete Stage 2 deployment without pretending the stage is complete.

For strict production evidence, render the persistent evidence bundle:

```bash
kubectl kustomize deploy/stage2-production-evidence --load-restrictor LoadRestrictionsNone
```

That bundle includes the all-up Stage 2 production gate plus narrower evidence Jobs for observability collector, Remote Computer, provider governance, tenant isolation, Vault/KMS, approval notifications, worker operations, policy rollout orchestration, Codex App Server, MCP Gateway, eval/release rollout, and finance operations. The observability Job runs `scripts/observability-collector-evidence-gate.sh`, calls the collector readiness plus deployment/cluster rollout validation endpoints, optionally runs remediation supervision, and writes evidence under the shared production evidence PVC for later archival. The Remote Computer Job runs `scripts/remote-computer-evidence-gate.sh`, calls the readiness/runner/state-sync validation endpoints, optionally records sidecar recovery evidence, and writes its output under the same PVC. The provider governance Job runs `scripts/provider-governance-evidence-gate.sh`, calls the provider summary/policy/deployment gates, optionally records provider rollout/rollback evidence, and writes its output under the same PVC. The tenant isolation Job runs `scripts/tenant-isolation-evidence-gate.sh`, calls readiness plus tenant routing validation, and writes its output under the same PVC. The Vault Job runs `scripts/vault-evidence-gate.sh`, calls readiness/health plus KMS recovery and rotation validation endpoints, and writes its output under the same PVC. The approval notification Job runs `scripts/approval-notification-evidence-gate.sh`, calls routing/run-history/deployment/ops validation endpoints, optionally records delivery evidence, and writes its output under the same PVC. The worker Job runs `scripts/worker-evidence-gate.sh`, calls worker readiness and load-validation endpoints, and writes its output under the same PVC. The policy rollout Job runs `scripts/policy-rollout-evidence-gate.sh`, calls orchestration readiness, validation, and due-run endpoints, and writes its output under the same PVC. The Codex App Server Job runs `scripts/codex-app-server-evidence-gate.sh`, calls health/control-plane/deployment/ops endpoints, optionally records stale-poll evidence, and writes its output under the same PVC. The MCP Gateway Job runs `scripts/mcp-gateway-evidence-gate.sh`, calls team-scoped rollout summary/history/deployment validation endpoints, optionally records due-run supervision evidence, and writes its output under the same PVC. The eval/release Job runs `scripts/eval-release-evidence-gate.sh`, calls release summary/history/deployment/orchestration validation endpoints, optionally records Stage 2 regression and due-run evidence, and writes its output under the same PVC. The finance Job runs `scripts/finance-evidence-gate.sh`, calls finance summary and operations readiness endpoints, optionally records finance close and accounting reconciliation evidence, and writes its output under the same PVC.
