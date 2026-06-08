# Ecommerce Platform Closed Loop

This document records the repo-controlled closed-loop contract for the ecommerce
DomainPack family. It is not a claim that every marketplace connector is
customer-grade production-ready. It defines what each platform pack must expose
before it can be promoted from draft-first pilot to controlled live operation.

## Platform Family

The ecommerce family is:

- `packs/ecommerce-core`
- `packs/ecommerce-tmall`
- `packs/ecommerce-taobao`
- `packs/ecommerce-xiaohongshu`
- `packs/ecommerce-tiktok-shop`
- `packs/ecommerce-amazon`

Every platform pack must extend `ecommerce-core`, keep `domain_scope:
ecommerce`, and keep connector results behind the untrusted-data boundary.

## Closed-Loop Contract

Each platform connector must declare:

- Native adapter runtime through `native.connector.call`.
- Dry-run support and `approval_commit_only` live execution.
- Read operations with request contracts, response contracts, and evidence IDs.
- Write operations with approval requirements and forbidden fields.
- Action files that bind to declared write operations.
- Approval commit binding over tenant, workspace, connector, API, operation,
  object, payload digest, and approval token.
- Production-readiness evidence requirements for sandbox/live separation, token
  lifecycle, rate-limit/retry policy, idempotency and reconciliation, webhook or
  polling ingestion, compensation or explicit non-compensable policy, approval
  boundary, and secret redaction.

The verification entrypoint is:

```bash
scripts/verify-ecommerce-platform-closed-loop.sh
```

The script writes inventory evidence under
`.mandoforge/ecommerce-platform-closed-loop/` and fails if any platform pack
loses the closed-loop contract.

## Runtime Boundary

The runtime readiness endpoint remains fail-closed:

```text
GET /api/native-connectors/production-readiness
```

If connector-specific customer-grade evidence is absent, the endpoint reports
blocked even though the repo-controlled pack contract validates. This preserves
the distinction between:

```text
pack closed-loop contract ready != customer live connector ready
```

Live execution still requires the shared live gate, platform credentials,
platform-specific control-plane URLs or policies, and a consumed approval commit
token.
