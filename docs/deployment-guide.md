# Stage 1 Deployment And Demo Guide

This guide verifies the Stage 1 runtime loop:

```text
create session -> run harness -> policy approval -> approve -> execute tool -> artifact -> replay/audit
```

## Local Runtime

Start the API:

```bash
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_ALLOW_HOST_SHELL_EXEC=1 \
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

## Agent OS Core Evidence

Agent OS core completion is gated by runtime action evidence. Against a running
API, collect the core evidence with:

```bash
BASE_URL=http://127.0.0.1:8787 ./scripts/agent-os-core-evidence-gate.sh
```

This gate fails unless the demo flow leaves evidence in `session_events`,
`tool_calls`, and `audit_logs`, and a managed `codex_cli` runtime adapter run
leaves `runtime_adapter.event` plus normalized `runtime.turn.*` evidence.

For a self-contained local rehearsal that starts an ephemeral API process first:

```bash
./scripts/stage1-final-gate.sh
```

The final gate uses the in-memory API store by default, starts a temporary API,
runs the Agent OS core evidence gate, verifies WorkItem collaboration intake
assignment routing, and review evidence, and verifies the Codex adapter shim. It
is the main local completion check for the runtime kernel and first
Collaboration Layer intake/routing/review slice.
