# Whiskey Adoption Runbook

This runbook turns `wishky-2-1` into a repeatable production-like adoption target for MandoForge.

It does not claim full production validation. Whiskey is a single-host pilot unless a real Kubernetes cluster, Codex App Server target, external controllers, and production integrations are configured.

For the latest lane status and residual backlog, see [Whiskey Adoption Status](whiskey-adoption-status.md).

For the fastest operator handoff from the latest local artifacts, run:

```bash
scripts/whiskey-adoption-next-actions.sh
```

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

The deploy script copies [docker-compose.adoption.yml](../deploy/whiskey/docker-compose.adoption.yml), the Whiskey Codex controller, the Whiskey tenant routing controller, the Whiskey worker load controller, the Whiskey MCP pilot controller, the Whiskey eval/release controller, the Whiskey observability controller, the Whiskey provider rollout controller, the Whiskey approval notification controller, the Whiskey Vault/KMS controller, and the Whiskey finance controller to the remote host, creates `/opt/mandoforge-adoption/whiskey.env` if missing, starts the loopback Codex WebSocket target plus controllers when needed, pulls the configured image, and starts the API, Postgres, and worker.

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
- runs `scripts/remote-computer-evidence-gate.sh` with sidecar recovery capture enabled so the archive records the audited no-op or replacement plan while preserving the state-sync production blocker;
- runs `scripts/whiskey-remote-computer-k3s-host-inventory.sh` locally and syncs the timestamped plus `-latest` k3s preflight, verify, and consolidated host-inventory artifacts into `/opt/mandoforge-adoption/evidence/remote-computer` and strict `stage2-production/remote-computer-k3s/`;
- seeds a Whiskey eval/release request, runs `scripts/eval-release-evidence-gate.sh`, and captures rollout, orchestration, deployment, and rollback evidence;
- seeds a diagnostics approval path for observability remediation, runs `scripts/observability-collector-evidence-gate.sh`, and captures deployment, rollout, remediation, plus `otel-collector` service and live-signal log evidence;
- seeds or refreshes the Whiskey provider rollout target, preferring the real DeepSeek-backed provider when `DEEPSEEK_API_KEY` is available on Whiskey and falling back to `whiskey-mock-provider` otherwise, then runs `scripts/provider-governance-evidence-gate.sh`;
- seeds a routable approval plus webhook policy, runs `scripts/approval-notification-evidence-gate.sh`, and captures deployment, ops, and delivery evidence;
- seeds a Vault secret catalog record, runs `scripts/vault-evidence-gate.sh`, and captures Vault health, KMS rotation, and KMS recovery controller evidence;
- runs `scripts/finance-evidence-gate.sh` with finance close, reconciliation, CSV export, and webhook delivery evidence enabled;
- runs the synced `scripts/workflow-pack-evidence-gate.sh` on the Whiskey host against the loopback API;
- seeds another Whiskey eval/release request, another observability remediation path, another Whiskey provider rollout path, another routable approval, another Vault secret catalog record, a due policy revision, and the static UI smoke prerequisites before running the synced `scripts/stage2-production-evidence-gate.sh` on the Whiskey host against the loopback API with `ALLOW_BLOCKED=1`, strict finance controller/export capture enabled, `RUN_STAGE2_POLICY_DUE_RUN=1`, `RUN_STAGE2_UI_STATIC_ASSETS=1`, and `RUN_STAGE2_UI_ACTIONBOOK=1`;
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
MANDOFORGE_TENANT_ROUTING_MODE=tenant_routed
MANDOFORGE_TENANT_ROUTING_CONTROLLER_URL=http://host.docker.internal:18790/tenant/routing/validate
```

The API now has an explicit tenant-routed runtime mode for Whiskey adoption. In that mode, request handling resolves `x-mandoforge-tenant-id` into a request-local tenant context, store queries bind that tenant, and Postgres connections refresh `mandoforge.tenant_id` on acquire. The controller still is not a production multi-tenant router: it validates the live Whiskey API readiness report against the payload sent by `/api/tenant-isolation/routing/validate`, verifies tenant header, RLS, and membership-scope signals, and preserves any remaining production blockers. This gives repeatable tenant-routed Whiskey evidence without claiming external enterprise multi-tenant adoption.

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

Without a real cluster, Remote Computer evidence is an inventory lane. The standard evidence script records readiness, runner, state-sync validation, and sidecar recovery evidence under `/evidence/remote-computer`, and `scripts/whiskey-remote-computer-k3s-host-inventory.sh` adds the host-side preflight plus verify inventory that is also copied into the strict archive under `stage2-production/remote-computer-k3s/`. On the single-host Whiskey pilot, sidecar recovery normally records a `noop` audited plan when no unhealthy sidecar heartbeat exists; it must not be treated as pod replacement proof. Production state sync should remain blocked until a distributed state filesystem and lock-aware state-sync manager are configured. Complete Remote Computer production evidence requires a real cluster or a `k3s` pilot on Whiskey.

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

The evidence script seeds a `whiskey-docs` connector on the Whiskey pilot team, keeps the `search` tool allowlisted, creates a due rollout when one is not already pending, enables strict MCP due-run plus rollback evidence, and routes `/v1/call` through the configured upstream mode.

The current Whiskey evidence was collected with:

```bash
WHISKEY_MCP_UPSTREAM_MODE=github_repo_contents
WHISKEY_MCP_GITHUB_REPO_OWNER=proerror77
WHISKEY_MCP_GITHUB_REPO_NAME=Goodchance
WHISKEY_MCP_GITHUB_REPO_REF=main
WHISKEY_WORKFLOW_PACK_MCP_QUERY=README
```

In that mode, `whiskey-docs` reads authenticated file contents from the private `proerror77/Goodchance` repository and returns path-level hits such as `clients/ios-app/README.md`. This validates the real MandoForge MCP gateway HTTP boundary and rollout controller hooks while exercising a credentialed internal repository knowledge target rather than a chat transcript.

The same controller also supports `WHISKEY_MCP_UPSTREAM_MODE=lark_chat_messages` and `WHISKEY_MCP_UPSTREAM_MODE=lark_docs_search`. `lark_docs_search` uses `lark-cli docs +search` against the current user's Docs/Wiki search scope; it is the next repo-native path for turning `whiskey-docs` into a broader Lark knowledge-space target, but it requires the Whiskey `lark-cli` login to have `search:docs:read`.

Use the repo-native helper below to check or start that scope upgrade:

```bash
scripts/whiskey-mcp-lark-docs-scope.sh
scripts/whiskey-mcp-lark-docs-scope.sh --capture-login-prompt
scripts/whiskey-mcp-lark-docs-scope.sh --start-login
```

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

When `DEEPSEEK_API_KEY` is available on Whiskey, the deploy script now syncs it into `whiskey.env`, the API container receives it, and the evidence script upgrades the seeded provider to `whiskey-deepseek-provider` with `base_url=https://api.deepseek.com`, `default_model=deepseek-v4-flash`, and `config.api_key_ref=vault:providers/whiskey-deepseek#api_key`. The referenced secret is created or reused through the Whiskey Vault boundary so provider health can resolve a Vault-backed key rather than an env-key warning path. When the DeepSeek key is unavailable, the seed falls back to `whiskey-mock-provider`.

The evidence script then runs the focused provider governance gate with `RUN_STAGE2_PROVIDER_ROLLOUT=1`, and the strict Stage 2 gate also records provider deployment validation, policy gate, rollout, and rollback evidence.

This is now a Whiskey real provider proof. It validates the provider governance policy gate, a healthy external `/v1/models` probe against DeepSeek, deployment-controller evidence, production rollout hook, and rollback hook against the live Whiskey API. It still does not claim that a broader multi-provider fleet, credential-rotation policy, or production traffic switch has been adopted.

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

On Whiskey, the approval webhook controller can run in `lark_im` mode and auto-discover the current `lark-cli` user `open_id` unless `WHISKEY_APPROVAL_NOTIFICATION_LARK_OPEN_ID` is set explicitly. In that mode, `/approval/webhook` forwards each accepted approval notification to a real Feishu/Lark private chat through `lark-cli im +messages-send`, and `/healthz` records the latest forwarded `message_id` and `chat_id` for evidence capture.

This is now a Whiskey real notification proof. It validates routing, channel policy, delivery through the live Whiskey API, deployment-controller evidence, ops-controller evidence, and a real Feishu/Lark delivery target. It is still not a claim that Slack, email, PagerDuty, or broader approval-group fan-out has been adopted.

## Vault/KMS Lane

Current Whiskey wiring starts a local Vault/KMS pilot controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_SECRET_PROVIDER=vault
MANDOFORGE_VAULT_ADDR=http://host.docker.internal:18797
MANDOFORGE_VAULT_MOUNT=kv
MANDOFORGE_KMS_PROVIDER=mock-kms
MANDOFORGE_KMS_KEY_ID=whiskey-kms-key-1
MANDOFORGE_KMS_ROTATION_POLICY=whiskey-manual-confirmed
MANDOFORGE_KMS_VALIDATION_MODE=external
MANDOFORGE_KMS_ENDPOINT=http://host.docker.internal:18797/kms/rotate
MANDOFORGE_KMS_RECOVERY_CONTROLLER_REQUIRED=true
MANDOFORGE_KMS_RECOVERY_CONTROLLER_URL=http://host.docker.internal:18797/kms/recovery/validate
```

The controller exposes a minimal Vault-compatible health and KV v2 surface plus KMS rotation and recovery validation endpoints. The evidence script seeds a catalog-only secret reference, runs the focused Vault gate with `RUN_STAGE2_SECRET_LIFECYCLE=1`, then includes the same lifecycle flag in the strict Stage 2 gate.

This is a Whiskey pilot KMS lifecycle proof. It validates the API's Vault provider health path, external KMS rotation boundary, recovery-controller boundary, audit trail, and archive coverage against the live Whiskey API. It is not a claim that a real HSM, enterprise Vault cluster, envelope-encryption policy, or secret rollback procedure has been adopted.

## Finance Accounting Lane

Current Whiskey wiring starts a local finance pilot controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_USAGE_EXPORT_SCHEDULE=true
MANDOFORGE_USAGE_EXPORT_WEBHOOK_URL=http://host.docker.internal:18798/finance/export
MANDOFORGE_FINANCE_CLOSE_CONTROLLER_REQUIRED=true
MANDOFORGE_FINANCE_CLOSE_CONTROLLER_URL=http://host.docker.internal:18798/finance/close
MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_REQUIRED=true
MANDOFORGE_FINANCE_RECONCILIATION_CONTROLLER_URL=http://host.docker.internal:18798/finance/reconcile
```

The controller accepts the API's finance export webhook payload, validates the close payload after rollup/export/alert prerequisites are ready, and validates reconciliation after close evidence exists. The evidence script runs the focused finance gate with:

```bash
RUN_STAGE2_FINANCE_CONTROLLERS=1
RUN_STAGE2_FINANCE_EXPORT=1
```

The strict Stage 2 gate also enables those flags so the archive contains `finance-close-evidence.json`, `finance-reconciliation-evidence.json`, `usage-export-csv-evidence.json`, and `finance-export-delivery-evidence.json`.

On Whiskey, the finance controller can run in `lark_drive` mode and upload the generated `mandoforge-usage-export.csv` artifact to a real Feishu Drive file through `lark-cli drive +upload`. The controller health endpoint records the latest uploaded `file_token`, `file_url`, and file name so evidence capture can prove that the webhook reached an external file target instead of only a local mock endpoint.

This is now a Whiskey real finance export proof. It validates the API's export delivery, finance-close controller boundary, reconciliation-controller boundary, audit trail, and archive coverage against the live Whiskey API, with the export artifact landing in Feishu Drive. It is still not a claim that a real ERP, billing ledger, or accounting system of record has been adopted.

## Observability Collector Lane

Current Whiskey wiring starts a local observability controller on the Docker gateway and configures the API with:

```bash
MANDOFORGE_SERVICE_NAME=mandoforge-api
MANDOFORGE_OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4318
MANDOFORGE_OTEL_COLLECTOR_HEALTH_ENDPOINT=http://otel-collector:13133/healthz
MANDOFORGE_OTEL_SAMPLE_RATIO=1.0
MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_REQUIRED=true
MANDOFORGE_OBSERVABILITY_COLLECTOR_DEPLOYMENT_CONTROLLER_URL=http://host.docker.internal:18794/observability/collector/deployment/validate
MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_REQUIRED=true
MANDOFORGE_OBSERVABILITY_COLLECTOR_CLUSTER_CONTROLLER_URL=http://host.docker.internal:18794/observability/collector/cluster/validate
MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_REQUIRED=true
MANDOFORGE_OBSERVABILITY_REMEDIATION_CONTROLLER_URL=http://host.docker.internal:18794/observability/remediation/run
```

The deploy script now also starts a real single-node `otel/opentelemetry-collector-contrib:0.123.0` service from [otel-collector-config.yaml](../deploy/whiskey/otel-collector-config.yaml). The API emits logs, traces, and metrics to that collector over the Compose network, while the host-side observability controller continues to provide deployment, cluster-rollout, and remediation validation endpoints.

The evidence script seeds a diagnostics session so remediation supervision has real pending-approval material to inspect, then runs the focused observability gate and the strict Stage 2 gate with `RUN_STAGE2_OBSERVABILITY_REMEDIATION=1`. It also captures `otel-collector-evidence.json`, `otel-collector-service.json`, and `otel-collector-live-signals.log` so the archive proves that the official collector service is running and receiving telemetry batches.

This is now a Whiskey real collector proof on a single node. It validates that the deployed API emits to a reachable official OTel collector service on Whiskey, that collector health is fresh, that logs/traces/metrics were received, and that the Stage 2 controller hooks are wired, fresh, and fail-closed. It is still not a claim that an external production collector cluster, retention backend, alert route, or cross-node telemetry pipeline has been adopted.

## WorkflowPack Lane

Current Whiskey evidence also exercises the Stage 3 WorkflowPack live API against the packaged AI Governance Pack:

```bash
BASE_URL=http://127.0.0.1:18787 \
EVIDENCE_DIR=/opt/mandoforge-adoption/evidence/workflow-packs \
WORKFLOW_PACK_MANIFEST_PATH=packs/ai-governance/package.yaml \
scripts/workflow-pack-evidence-gate.sh
```

The gate validates the manifest contract, proves onboarding fails closed with placeholder or missing customer input, installs the pack, confirms install bootstraps default onboarding profile assets, stages the installation, confirms release fails unless eval and release gates pass, releases the pack with explicit gate evidence, creates an immutable `0.1.1` update version, confirms the updated installation also boots default profile assets, persists customer onboarding profiles as versioned assets, proves onboarding reaches `ready` when those persisted assets plus connector declarations satisfy the pack contract, then proves connector quality fails closed with stale or incomplete samples and reaches `ready` with fresh attributable samples bound to the real Whiskey `whiskey-docs` MCP server and sourced from authenticated private repository content reads. The current pilot result is a real hit from `proerror77/Goodchance@main`, with `clients/ios-app/README.md` captured as the live connector-quality sample. This is a Whiskey pilot proof for the WorkflowPack install/stage/release lifecycle plus contract-level onboarding readiness, install defaults, profile-asset persistence, connector-quality gating, real MCP server binding, and a credentialed non-pilot enterprise read source; it does not yet prove broader Lark docs/wiki spaces or external SaaS connectors such as Slack, Jira, or Confluence.

Before installing `k3s`, run the host preflight first:

```bash
scripts/whiskey-remote-computer-k3s-preflight.sh
```

If the preflight result is accepted, the next repo-native command is:

```bash
scripts/whiskey-remote-computer-k3s-prepare.sh
```

That script defaults to `dry_run` and only prints the exact changes it would make on Whiskey:

- `modprobe br_netfilter`
- write `/etc/modules-load.d/mandoforge-remote-computer.conf`
- write `/etc/sysctl.d/99-mandoforge-remote-computer.conf`
- run `sysctl --system`

Use `--apply` only after explicitly approving a constrained k3s pilot.

After that preparation step, the install runway is:

```bash
scripts/whiskey-remote-computer-k3s-install.sh
```

This also defaults to `dry_run`. On Whiskey today it reports the exact install plan:

- `curl -sfL https://get.k3s.io | INSTALL_K3S_CHANNEL=stable INSTALL_K3S_EXEC="server --disable=traefik --write-kubeconfig-mode=644 --kube-apiserver-arg=service-node-port-range=30080-30443" sh -`
- `systemctl enable --now k3s`

Use `--apply` only after the preflight and preparation outputs are reviewed and the constrained pilot is explicitly approved.

After installation, verify the single-node pilot with:

```bash
scripts/whiskey-remote-computer-k3s-verify.sh
```

Before approval, the current Whiskey host returns `status=not_installed` with no reserved port collisions and no kubeconfig or Ready nodes, which is the expected baseline.

The latest preflight on 2026-05-17 returned `status=constrained_pilot_only`. It confirmed that Whiskey is still a 2 vCPU / 3.4 GiB RAM host with only about 1.6 GiB immediately available memory, 1.4 GiB of swap already in use, no current k3s-reserved port collisions, and missing `br_netfilter` plus `bridge-nf-call-iptables`. That means cluster work should stay isolated from existing services and only proceed as an explicit constrained experiment.

Current Whiskey capacity snapshot from the adoption check:

- 2 vCPU.
- 3.4 GiB RAM, with roughly 1.6 GiB available during the latest preflight.
- 4.0 GiB swap, with existing swap use observed.
- No `k3s` or `kubectl` installed.
- Existing public services already use ports `5432`, `8080`, `3000`, and `9377`; MandoForge remains loopback-only on `18787` and `15432`.
- `br_netfilter` is not loaded and `net.bridge.bridge-nf-call-iptables` is missing.

Recommendation: keep Whiskey in single-host pilot mode unless explicitly deciding to spend this host's remaining headroom on a constrained single-node `k3s` pilot. If `k3s` is installed later, first load `br_netfilter`, enable bridge iptables, cap Remote Computer warm-pool replicas, and keep Codex/Remote Computer services on loopback or cluster-internal networking until an authenticated ingress policy exists.
