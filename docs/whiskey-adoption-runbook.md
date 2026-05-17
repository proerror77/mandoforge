# Whiskey Adoption Runbook

This runbook turns `wishky-2-1` into a repeatable production-like adoption target for MandoForge.

It does not claim full production validation. Whiskey is a single-host pilot unless a real Kubernetes cluster, Codex App Server target, external controllers, and production integrations are configured.

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

The deploy script copies [docker-compose.adoption.yml](../deploy/whiskey/docker-compose.adoption.yml) to the remote host, creates `/opt/mandoforge-adoption/whiskey.env` if missing, pulls the configured image, and starts the API, Postgres, and worker.

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
- runs `scripts/scheduler-evidence-gate.sh`;
- runs `scripts/codex-app-server-evidence-gate.sh`;
- runs `scripts/stage2-production-evidence-gate.sh` with `ALLOW_BLOCKED=1`;
- archives Stage 2 evidence and full pilot evidence under `/opt/mandoforge-adoption/archives`;
- syncs archive copies to `.mandoforge/remote-adoption/whiskey/`;
- verifies the Stage 2 archive locally with `ALLOW_BLOCKED=1`.

Use strict production validations only after real controller URLs and credentials are configured:

```bash
RUN_STAGE2_PRODUCTION_VALIDATIONS=1 scripts/whiskey-adoption-evidence.sh
```

If strict validation fails on a missing controller URL, that is the expected fail-closed behavior. Do not replace it with a mock controller for production adoption evidence.

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

## Worker And Remote Computer Lane

Whiskey can collect single-host worker readiness evidence without Kubernetes. Complete Remote Computer production evidence requires a real cluster or a `k3s` pilot on Whiskey.

Before installing `k3s`, decide whether Whiskey should carry that operational burden. The host has limited memory, so cluster work should be isolated from existing services and measured before claiming Remote Computer production readiness.
