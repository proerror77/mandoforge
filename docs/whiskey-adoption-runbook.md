# Whiskey Adoption Runbook

This runbook turns `wishky-2-1` into a repeatable production-like adoption target for MandoForge.

It does not claim full production validation. Whiskey is a single-host pilot unless a real Kubernetes cluster, Codex App Server target, external controllers, and production integrations are configured.

For the latest lane status and residual backlog, see [Whiskey Adoption Status](whiskey-adoption-status.md).

## Host Contract

- SSH host: `wishky-2-1`.
- Remote root: `/opt/mandoforge-adoption`.
- Compose project: `mandoforge-adoption`.
- API port: `127.0.0.1:18787`.
- Postgres port: `127.0.0.1:15432`.
- Evidence root: `/opt/mandoforge-adoption/evidence`.
- Archive root: `/opt/mandoforge-adoption/archives`.
- Local archive sync: `.mandoforge/remote-adoption/whiskey/`.

Keep the API bound to loopback unless a separate authenticated ingress is added.

## Deploy

Publish or select an image tag first:

```bash
MANDOFORGE_IMAGE_TAG=<tag> scripts/whiskey-adoption-deploy.sh
```

The deploy script copies [docker-compose.adoption.yml](../deploy/whiskey/docker-compose.adoption.yml), the Whiskey Codex controller, the Whiskey tenant routing controller, the Whiskey worker load controller, the Whiskey MCP pilot controller, the Whiskey eval/release controller, the Whiskey observability controller, the Whiskey provider rollout controller, and the Whiskey approval notification controller to the remote host, creates `/opt/mandoforge-adoption/whiskey.env` if missing, starts the loopback Codex WebSocket target plus controllers when needed, pulls the configured image, and starts the API, Postgres, and worker.

The default image is:

```text
ghcr.io/proerror77/mandoforge/mandoforge-api:${MANDOFORGE_IMAGE_TAG:-latest}
```

Do not store real controller tokens in git. Put them in `/opt/mandoforge-adoption/whiskey.env` on the remote host.

## Evidence

Run the standard blocked-inventory evidence lane:

```bash
scripts/whiskey-adoption-evidence.sh
```

The script:

- verifies `GET /healthz`;
- ensures `Whiskey Adoption Org` and `Whiskey Pilot Team` exist;
- syncs repo-local Stage 2 evidence scripts and controller manifests to `/opt/mandoforge-adoption/`;
- runs `scripts/scheduler-evidence-gate.sh`;
- runs `scripts/codex-app-server-evidence-gate.sh`;
- runs `scripts/tenant-isolation-evidence-gate.sh`;
- runs `scripts/worker-evidence-gate.sh`;
- runs `scripts/remote-computer-evidence-gate.sh`;
- seeds a Whiskey eval/release request, runs `scripts/eval-release-evidence-gate.sh`, and captures rollout, orchestration, deployment, and rollback evidence;
- seeds a diagnostics approval path for observability remediation, runs `scripts/observability-collector-evidence-gate.sh`, and captures deployment, rollout, and remediation evidence;
- seeds a Whiskey mock provider, runs `scripts/provider-governance-evidence-gate.sh`, and captures provider deployment, policy-gate, rollout, and rollback evidence;
- seeds a routable approval plus webhook policy, runs `scripts/approval-notification-evidence-gate.sh`, and captures deployment, ops, and delivery evidence;
- runs the synced `scripts/workflow-pack-evidence-gate.sh` on the Whiskey host against the loopback API;
- seeds another Whiskey eval/release request, another observability remediation path, another Whiskey mock provider, and another routable approval before running the synced `scripts/stage2-production-evidence-gate.sh` on the Whiskey host against the loopback API with `ALLOW_BLOCKED=1`;
- archives Stage 2 evidence and full pilot evidence under `/opt/mandoforge-adoption/archives`;
- syncs archive copies to `.mandoforge/remote-adoption/whiskey/`;
- verifies the Stage 2 archive locally with `ALLOW_BLOCKED=1`.

Use strict production validations only after real controller URLs and credentials are configured:

```bash
RUN_STAGE2_PRODUCTION_VALIDATIONS=1 scripts/whiskey-adoption-evidence.sh
```

If strict validation fails on a missing controller URL, that is the expected fail-closed behavior. Do not replace it with a mock controller for production adoption evidence.

## Tenant Routing Lane

Current Whiskey wiring starts a local tenant routing controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_TENANT_ROUTING_CONTROLLER_REQUIRED=true
MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL=http://host.docker.internal:18790/tenant/routing/validate
```

The controller is not a production multi-tenant router. It validates the live Whiskey API readiness report against the payload sent by `/api/tenant-isolation/routing/validate`, verifies fail-closed tenant headers and membership-scope signals, and preserves reported production blockers for single-runtime tenant routing and incomplete RLS. This gives repeatable controller evidence for Whiskey without marking the tenant routing lane production-ready.

## Codex App Server Lane

To move the Codex App Server backlog item from blocked to passed, configure these values in `/opt/mandoforge-adoption/whiskey.env`:

```bash
MANDOFORGE_CODEX_APP_SERVER_URL=<real app server url>
MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL=<real deployment controller url>
MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_TOKEN=<secret>
MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_REQUIRED=true
MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL=<real ops controller url>
MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_TOKEN=<secret>
```

Then redeploy and collect evidence:

```bash
scripts/whiskey-adoption-deploy.sh
scripts/whiskey-adoption-evidence.sh
```

Completion evidence must show `codex_app_server_health_status` healthy, deployment readiness unblocked, ops readiness unblocked, and a verified archive.

If the evidence script reports `Codex App Server is disabled until MANDOFORGE_CODEX_APP_SERVER_URL is configured`, the lane is still blocked and should stay in the external production adoption backlog.

Current Whiskey wiring uses a loopback Codex WebSocket target plus a local HTTP deployment/ops controller:

```bash
MANDOFORGE_CODEX_APP_SERVER_URL=ws://host.docker.internal:18788
MANDOFORGE_CODEX_APP_SERVER_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18789/deployment/validate
MANDOFORGE_CODEX_APP_SERVER_OPS_CONTROLLER_URL=http://host.docker.internal:18789/ops/validate
```

The controller validates by performing a real JSON-RPC `initialize` handshake against the Codex WebSocket App Server. The WebSocket adapter currently covers deployment and ops health evidence; full steering over native WebSocket remains a follow-up because thread/turn/command REST operations still require the HTTP adapter path.

## Worker And Remote Computer Lane

Whiskey can collect single-host worker readiness evidence without Kubernetes. The standard evidence script records worker readiness and load-validation evidence under `/evidence/worker`, then includes it in the full pilot archive.

Current Whiskey wiring starts a local worker load controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_REQUIRED=true
MANDOFORGE_WORKER_LOAD_VALIDATION_CONTROLLER_URL=http://host.docker.internal:18791/worker/load/validate
```

The controller validates the live Whiskey API worker-readiness report, durable Postgres queue mode, queue-backed worker mode, manifest hardening/autoscaling signals, the running Docker Compose worker service, and absence of failed jobs or stale leases. This is a Whiskey single-host worker validation, not a k3s multi-replica autoscaling proof.

Without a real cluster, Remote Computer evidence is an inventory lane. The standard evidence script records readiness, runner, and state-sync validation evidence under `/evidence/remote-computer`, but production state sync should remain blocked until a distributed state filesystem and lock-aware state-sync manager are configured. Complete Remote Computer production evidence requires a real cluster or a `k3s` pilot on Whiskey.

## MCP Connector Lane

Current Whiskey wiring starts a local MCP pilot gateway/controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_MCP_GATEWAY_URL=http://host.docker.internal:18792
MANDOFORGE_MCP_ALLOWED_SERVERS=whiskey-docs
MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_MCP_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18792/mcp/deployment/validate
MANDOFORGE_MCP_ROLLOUT_CONTROLLER_REQUIRED=true
MANDOFORGE_MCP_ROLLOUT_CONTROLLER_URL=http://host.docker.internal:18792/mcp/rollout/approve
MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_REQUIRED=true
MANDOFORGE_MCP_ROLLOUT_ROLLBACK_CONTROLLER_URL=http://host.docker.internal:18792/mcp/rollback/validate
```

The evidence script seeds a `whiskey-docs` connector on the Whiskey pilot team, keeps the `search` tool allowlisted, creates a due rollout when one is not already pending, and enables strict MCP due-run plus rollback evidence. This validates the real MandoForge MCP gateway HTTP boundary and rollout controller hooks without requiring an external SaaS connector.

## Eval/Release Lane

Current Whiskey wiring starts a local agent release rollout/deployment/orchestration/rollback controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_AGENT_RELEASE_CONTROLLER_REQUIRED=true
MANDOFORGE_AGENT_RELEASE_CONTROLLER_URL=http://host.docker.internal:18793/agents/releases/rollout/apply
MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_AGENT_RELEASE_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18793/agents/releases/deployment/validate
MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_REQUIRED=true
MANDOFORGE_AGENT_RELEASE_ORCHESTRATION_CONTROLLER_URL=http://host.docker.internal:18793/agents/releases/orchestration/validate
MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_REQUIRED=true
MANDOFORGE_AGENT_RELEASE_ROLLBACK_CONTROLLER_URL=http://host.docker.internal:18793/agents/releases/rollout/rollback
```

The evidence script bootstraps the Stage 2 regression suite, runs it against the Whiskey pilot agent, creates an auto-approved `whiskey-eval-release` request, runs the due-release automation, validates orchestration and deployment through the controller, then rolls the promoted release back. This is a Whiskey pilot release target proof; external production release targets still need their own controller credentials and promotion/rollback policy.

## Provider Rollout Lane

Current Whiskey wiring starts a local provider rollout controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_PROVIDER_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18795/provider/deployment/validate
MANDOFORGE_PROVIDER_ROLLOUT_CONTROLLER_URL=http://host.docker.internal:18795/provider/rollout/apply
MANDOFORGE_PROVIDER_ROLLOUT_ROLLBACK_CONTROLLER_URL=http://host.docker.internal:18795/provider/rollout/rollback
```

The evidence script seeds an active `mock` provider named `whiskey-mock-provider` with a daily request budget, then runs the focused provider governance gate with `RUN_STAGE2_PROVIDER_ROLLOUT=1`. The strict Stage 2 gate also runs provider deployment validation, policy gate, rollout, and rollback evidence.

This is a Whiskey pilot provider target proof. It validates the provider governance policy gate, deployment-controller hook, production rollout hook, and rollback hook against the live Whiskey API. It does not claim that an external OpenAI-compatible provider fleet, credential-rotation process, or production traffic switch has been adopted.

## Approval Notification Lane

Current Whiskey wiring starts a local approval notification controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_APPROVAL_WEBHOOK_URL=http://host.docker.internal:18796/approval/webhook
MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_APPROVAL_NOTIFICATION_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18796/approval-notification/deployment/validate
MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_REQUIRED=true
MANDOFORGE_APPROVAL_NOTIFICATION_OPS_CONTROLLER_URL=http://host.docker.internal:18796/approval-notification/ops/validate
```

The evidence script seeds an active webhook notification policy, rejects stale pending approvals that have no delegated approver or group target, creates a fresh pending approval delegated to `whiskey-approver`, and runs the focused approval notification gate with `RUN_STAGE2_APPROVAL_DELIVERY=1`. The strict Stage 2 gate also records deployment, ops, and delivery evidence.

This is a Whiskey pilot notification proof. It validates routing, channel policy, webhook delivery, deployment-controller evidence, and ops-controller evidence against the live Whiskey API. It is not a claim that an external Slack, email, PagerDuty, or enterprise notification provider has been adopted.

## Observability Collector Lane

Current Whiskey wiring starts a local observability controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_SERVICE_NAME=mandoforge-api
MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT=http://host.docker.internal:18794
MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT=http://host.docker.internal:18794/healthz
MANDOFORGE_OTEL_SAMPLE_RATIO=1.0
MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18794/observability/collector/deployment/validate
MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED=true
MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL=http://host.docker.internal:18794/observability/collector/cluster/validate
MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED=true
MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL=http://host.docker.internal:18794/observability/remediation/run
```

The controller exposes a small OTLP-compatible pilot target for `/v1/logs`, `/v1/traces`, and `/v1/metrics`, plus deployment, cluster-rollout, and remediation validation endpoints. The evidence script seeds a diagnostics session so remediation supervision has real pending-approval material to inspect, then runs the focused observability gate and the strict Stage 2 gate with `RUN_STAGE2_OBSERVABILITY_REMEDIATION=1`.

This is a Whiskey pilot collector proof. It validates that the deployed API emits to a reachable collector target and that the Stage 2 controller hooks are wired, fresh, and fail-closed. It is not a claim that an external production collector cluster, retention backend, alert route, or cross-node telemetry pipeline has been adopted.

## WorkflowPack Lane

Current Whiskey evidence also exercises the Stage 3 WorkflowPack live API against the packaged AI Governance Pack:

```bash
BASE_URL=http://127.0.0.1:18787 \
EVIDENCE_DIR=/opt/mandoforge-adoption/evidence/workflow-packs \
WORKFLOW_PACK_MANIFEST_PATH=packs/ai-governance/package.yaml \
scripts/workflow-pack-evidence-gate.sh
```

The gate validates the manifest contract, installs the pack, confirms release fails before staging, stages the installation, confirms release fails unless eval and release gates pass, then releases the pack with explicit gate evidence. This is a Whiskey pilot proof for the WorkflowPack install/stage/release lifecycle; it does not yet prove customer-specific pack onboarding quality or external connector data quality.

Before installing `k3s`, decide whether Whiskey should carry that operational burden. The host has limited memory, so cluster work should be isolated from existing services and measured before claiming Remote Computer production readiness.

Current Whiskey capacity snapshot from the adoption check:

- 2 vCPU.
- 3.4 GiB RAM, with roughly 1.9 GiB available during the pilot.
- 4.0 GiB swap, with existing swap use observed.
- No `k3s` or `kubectl` installed.
- Existing public services already use ports `5432`, `8080`, `3000`, and `9377`; MandoForge remains loopback-only on `18787` and `15432`.

Recommendation: keep Whiskey in single-host pilot mode unless explicitly deciding to spend this host's remaining headroom on a constrained single-node `k3s` pilot. If `k3s` is installed later, cap Remote Computer warm-pool replicas and keep Codex/Remote Computer services on loopback or cluster-internal networking until an authenticated ingress policy exists.
