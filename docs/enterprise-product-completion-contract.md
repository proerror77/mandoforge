# Enterprise Product Completion Contract

This contract defines what must be true before MandoForge can be called a full
enterprise product instead of a repo-controlled or production-like pilot.

The current repository has a strong Agent OS kernel, governed runtime, Workflow
Pack substrate, Context OS semantic layer, and ecommerce live-adapter path. That
is not the same as full enterprise product completion. Enterprise completion
requires production evidence across runtime, deployment, security, ontology,
connectors, workflow-pack operations, and operator surfaces.

## Completion Rule

MandoForge is enterprise-product-complete only when every required lane below is
`ready` against a customer-grade deployment target and the evidence is archived,
replayable, and less than the accepted freshness window for that target.

The rule is intentionally stricter than the Stage 1/Stage 2 repo-controlled
completion audits:

```text
pilot ready != enterprise product complete
single-node production-like evidence != multi-node production completion
generic connector governance != platform-specific live connector completion
ontology-ready semantic storage != ontology engine completion
```

## Evidence Classes

| Class | Meaning |
| --- | --- |
| `repo_controlled` | Local tests, contract validation, and deterministic API gates in this repo. |
| `production_like_pilot` | Archived evidence from a constrained but repeatable deployment such as Whiskey. |
| `customer_grade` | Evidence from a production-capable deployment target with durable infrastructure, real identity, real connector credentials, operational alerts, backup/restore, and support ownership. |

Enterprise completion requires `customer_grade` evidence for required lanes.

## Required Lanes

### runtime-production

The runtime must prove that managed sessions, event logs, tool calls, approvals,
artifacts, audits, and worker jobs remain durable across real operational
failure modes.

Required evidence:

- Postgres migrations are reversible or forward-recoverable with documented
  rollback policy.
- Backup and restore drills preserve sessions, events, approvals, tool calls,
  artifacts, audit logs, workflow runs, semantic objects, and context packets.
- Session-loop jobs and execution jobs recover after API restart, worker
  restart, queue restart, and partial tool failure.
- Every external side effect has an idempotency key or equivalent duplicate
  prevention mechanism.
- Dead-letter, manual replay, and audit trail paths exist for failed jobs.
- SSE replay and timeline APIs work under reconnect and backpressure tests.

Primary repo surfaces:

- `docs/architecture.md`
- `docs/runtime-truth-audit.md`
- `GET /api/enterprise-product/readiness`
- `scripts/enterprise-product-readiness-gate.sh`
- `scripts/agent-os-core-evidence-gate.sh`
- `scripts/managed-session-runtime-evidence-gate.sh`
- `scripts/worker-evidence-gate.sh`

### remote-computer-multinode

Remote Computer must be production-capable beyond a single-node local-hostpath
pilot.

Required evidence:

- Remote Computer state uses distributed RWX storage such as JuiceFS, CephFS,
  Longhorn RWX, or an equivalent production storage class.
- Workspace, notes, memory, skills, and artifacts use lock-aware sync semantics.
- Warm-pool pods, assigned pods, sidecars, leases, and recovery controllers work
  across at least two schedulable nodes.
- Pod execution cannot bypass Tool Router, Policy, Approval, Audit, or timeline
  recording.
- NetworkPolicy, resource limits, cleanup, and tenant isolation are enforced.
- Sidecar heartbeat, stale lease cleanup, and replacement recovery are evidenced.

Primary repo surfaces:

- `docs/agent-remote-computer-plan.md`
- `docs/whiskey-adoption-completion-audit.md`
- `GET /api/remote-computers/production-path`
- `scripts/remote-computer-evidence-gate.sh`
- `scripts/worker-remote-computer-evidence-gate.sh`
- `scripts/whiskey-remote-computer-k3s-verify.sh`

Current boundary:

Whiskey single-node k3s evidence is useful production-like pilot evidence, but it
does not close this lane without distributed state and multi-node recovery proof.

### live-connector-production

Native and MCP connectors must prove platform-specific production semantics, not
only generic approval gating.

Required evidence:

- Each live connector has sandbox and live environment separation.
- OAuth, token refresh, secret rotation, and credential expiry are handled.
- Rate limits, retries, backoff, timeout, and platform-specific error taxonomy
  are implemented and tested.
- External writes are approval-gated, idempotent, and reconcile against the
  external system after execution.
- Compensation or rollback adapters are defined where the platform supports
  them, and explicit non-compensable operations are marked.
- Webhook or polling ingestion records external state changes with provenance.
- Connector results never leak raw secrets into tool results, artifacts, events,
  logs, or audit rows.

Primary repo surfaces:

- `docs/workflow-pack-manifest-contract.md`
- `docs/ecommerce-platform-closed-loop.md`
- `crates/mandoforge-api/src/native_connectors.rs`
- `crates/mandoforge-api/src/execution.rs`
- `GET /api/native-connectors/production-readiness`
- `scripts/native-connector-production-readiness-gate.sh`
- `scripts/verify-ecommerce-platform-closed-loop.sh`
- `scripts/verify-ecommerce-tmall-context-os.sh`
- `scripts/workflow-pack-evidence-gate.sh`

Required platform promotion:

- Tmall/Taobao TOP
- Xiaohongshu Shop
- TikTok Shop Open API
- Amazon Selling Partner API
- Lark/Feishu MCP and native enterprise connectors

### ontology-engine

The semantic layer must become a versioned ontology service with reviewable
changes and runtime enforcement.

Required evidence:

- Core ontology objects and relations are versioned and queryable.
- Domain ontologies are versioned per pack or domain and can be promoted,
  rolled back, and migrated.
- Ontology relation constraints are enforced before policy decisions rely on
  semantic links.
- Ontology Builder creates reviewable proposals and approved ontology changes
  create durable audit evidence.
- Conflicts, contradictions, stale facts, and trust-level downgrades are visible
  to operators and block high-risk actions when required.
- Context packets can be rendered with exact source refs, ontology version,
  relation expansion, and trust/freshness gates.

Primary repo surfaces:

- `docs/drafts/context-os-memory-architecture.md`
- `docs/architecture.md`
- `crates/mandoforge-api/src/store_semantic_kernel.rs`
- `crates/mandoforge-api/src/store_context_packets.rs`
- `GET /api/ontology/engine-readiness`
- `scripts/ontology-engine-readiness-gate.sh`
- `scripts/verify-ecommerce-tmall-context-os.sh`

Current boundary:

The repo is ontology-ready and has Context OS primitives. Enterprise completion
requires a promoted ontology registry, domain ontology lifecycle, migration
policy, and operator-facing release workflow.

### workflowpack-enterprise-lifecycle

Workflow Packs must be operational products, not only valid manifests.

Required evidence:

- Install, stage, release, rollback, archive, update, and onboarding are covered
  by API gates and audit records.
- Tenant onboarding has completeness checks and customer-specific profile
  versioning.
- Connector quality gates run against real tenant connector accounts.
- Pack release gates include eval regression, policy readiness, connector
  readiness, approval policy, and rollback readiness.
- Pack updates preserve compatibility and migration evidence.
- Released packs can be canaried, rolled back, and compared across versions.

Primary repo surfaces:

- `docs/workflow-pack-manifest-contract.md`
- `crates/mandoforge-api/src/workflow_pack.rs`
- `scripts/verify-workflow-pack-manifest.sh`
- `scripts/workflow-pack-evidence-gate.sh`
- `scripts/managed-workflow-runtime-evidence-gate.sh`

### enterprise-security-admin

Enterprise controls must be complete enough for a real customer security review.

Required evidence:

- SSO via OIDC/SAML and user provisioning via SCIM or equivalent directory sync.
- RBAC/ABAC for tenant, org, team, project, agent, workflow, connector, memory,
  approval, and audit access.
- Tenant isolation and RLS are enforced in production mode.
- Vault/KMS/HSM integration uses a production-capable backend, with rotation and
  recovery evidence.
- Audit export supports SIEM ingestion.
- Data retention, legal hold, export, deletion, PII redaction, and DLP policies
  are documented and tested.
- Break-glass access, delegated approval, and escalation paths are audited.

Primary repo surfaces:

- `GET /api/enterprise-security/admin-readiness`
- `scripts/enterprise-security-admin-readiness-gate.sh`
- `scripts/tenant-isolation-evidence-gate.sh`
- `scripts/vault-evidence-gate.sh`
- `scripts/approval-notification-evidence-gate.sh`
- `scripts/stage2-production-evidence-preflight.sh`

### observability-ops

The product must be supportable by an operations team.

Required evidence:

- Metrics, traces, logs, audit trails, and cost data are correlated by tenant,
  session, workflow run, tool call, worker, connector, and provider.
- Alerts exist for failed jobs, stale leases, delivery failures, connector
  degradation, provider degradation, budget breach, and queue backlog.
- Deployment version, migration version, pack version, ontology version, and
  connector version are visible in operations surfaces.
- Incident timeline and manual repair actions are auditable.
- SLOs and operational runbooks exist for runtime, connector, worker, approval,
  and Remote Computer incidents.

Primary repo surfaces:

- `scripts/observability-collector-evidence-gate.sh`
- `scripts/finance-evidence-gate.sh`
- `scripts/stage2-production-evidence-gate.sh`
- `docs/deployment-guide.md`

### product-surfaces

The enterprise product must expose real operator and builder workflows through
live APIs.

Required evidence:

- Admin Console covers tenants, teams, agents, runtime profiles, providers,
  policies, approvals, connectors, budgets, and release state.
- Operator Console covers blocked work, approvals, runs, replay, artifacts,
  execution jobs, session-loop jobs, and manual repair.
- Builder Console covers Workflow Pack configuration, Ontology Builder,
  connector mapping, eval gates, and release gates.
- Ops Console covers health, workers, queues, costs, alerts, deployments, and
  incident evidence.
- UI truth gates prove these surfaces read live APIs and do not present fake
  completion state.

Primary repo surfaces:

- `scripts/verify-static-ui-assets.sh`
- `scripts/verify-ui-api-truth-gate.mjs`
- `scripts/verify-static-ui-actionbook.sh`
- `GET /api/enterprise-product/readiness`
- `scripts/managed-workflow-runtime-evidence-gate.sh`

## Promotion Sequence

1. Keep Agent OS core and Workflow Pack contracts green.
2. Close Remote Computer multi-node distributed state.
3. Promote ecommerce live connectors from generic live-call proof to
   platform-specific production semantics.
4. Promote Context OS into a versioned Ontology Engine with release workflow.
5. Add enterprise identity, security, audit export, and data-governance controls.
6. Prove operations readiness with SLOs, alerts, runbooks, and repair evidence.
7. Run a customer-grade enterprise completion archive and verify it with a
   fail-closed gate.

## Status Policy

Status labels must use these meanings:

- `ready`: Required customer-grade evidence exists and is fresh.
- `pilot_ready`: Repo-controlled or production-like evidence exists, but at least
  one customer-grade requirement is missing.
- `blocked`: A required production capability or environment is missing.
- `not_started`: The lane has no meaningful implementation or evidence.

The product may be described as `pilot_ready` while any required lane is
`pilot_ready` or `blocked`. It may be described as `enterprise_product_complete`
only when every required lane is `ready`.
