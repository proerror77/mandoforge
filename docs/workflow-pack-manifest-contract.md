# WorkflowPack Manifest Contract

This is the Stage 3 contract for installable Workflow Packs and Domain Packs.

The first implementation is intentionally contract-first: it validates package shape, file references, connector trust boundaries, worker roles, handoff rules, eval gates, and release gates before any runtime install API is allowed to mutate tenant behavior.

## Files

- Manifest schema: `schemas/workflow-pack-manifest.schema.json`
- Rust validator: `crates/mandoforge-api/src/workflow_pack.rs`
- Example pack: `packs/ai-governance/package.yaml`
- Verification entrypoint: `scripts/verify-workflow-pack-manifest.sh`

## Manifest Requirements

Every pack manifest must declare:

- `schema_version`: currently `workflowpack.mandoforge.dev/v1`.
- `kind`: `WorkflowPack` or `DomainPack`.
- `id`, `name`, `version`, and `description`.
- Capabilities.
- Profiles, skills, workflows, agents, connectors, schemas, policies, evals, and release gates.
- Onboarding workflow, required tenant profiles, profile schemas, and an onboarding eval.

All referenced files must be relative to the package directory, must exist, and must not use absolute paths or `..` escapes.

## Safety Rules

The validator enforces the initial Stage 3 safety floor:

- At least one workflow, agent, connector, required eval gate, and required release gate must exist.
- At least one profile, skill, schema, and policy must exist.
- The onboarding workflow must reference a declared workflow.
- Onboarding required profiles must reference declared profile ids.
- Onboarding profile schemas must be relative, existing files.
- The onboarding eval must reference a declared eval.
- Connector writes must be approval-gated when enabled.
- Connectors must require provenance.
- Connectors must declare tenant and workspace scope as required.
- Connector outputs must be treated as data, not instructions.
- Reader agents cannot declare write or external-write tool scopes.
- Writer agents cannot declare external-write tool scopes.
- Handoffs must target declared agents.
- Handoffs must declare enum-like intents.
- High-risk handoffs must require approval.
- Eval gate scores must be between `0` and `1`.

This contract does not install or activate packs yet. It blocks unsafe package shape before the later install/stage/release APIs exist.

## Lifecycle Semantics

### Install

Install should parse and validate the manifest, copy the package into a tenant-scoped immutable package store, and create a draft pack version. It must not create active agents, connectors, policies, schedules, or external writes by default.

### Update

Update should create a new pack version. Existing released versions remain immutable. A new version must pass manifest validation, pack evals, policy gates, and release approval before tenant behavior changes.

The initial Stage 3 update API implements the immutable-version contract:

- `POST /api/workflow-packs/installations/{id}/update` only accepts active `released` or `rolled_back` source installations.
- The update manifest must validate against the same package contract as install.
- The update manifest must keep the same pack `id` and `kind` as the source installation and must declare a different `version`.
- Update creates a separate `installed` installation with pending eval and release gates. It does not mutate the source installation, release timestamp, validation report, manifest snapshot, or rollback evidence.
- The new installation records source provenance under `gate_evidence.version_update` with the source installation id, source status, source version, reason, and creation timestamp.
- `workflow_pack.version_created` audit evidence records the source installation id and the new version.
- The new version must still pass stage and release gates before tenant behavior changes.

### Stage

Stage should materialize draft agent versions, connector definitions, policy revisions, eval suites, and profile requirements in a non-production state. Staging must preserve tenant scope and must not bypass provider, tool, approval, MCP, or audit governance.

### Onboarding Assessment

Customer onboarding must be explicit before a pack is treated as tenant-ready. The initial Stage 3 onboarding assessment API is contract-driven and non-destructive:

- `POST /api/workflow-packs/installations/{id}/onboarding/assess` accepts active pack installations and evaluates customer-supplied onboarding profiles plus connector declarations against the installed manifest snapshot.
- The assessment verifies required profiles are present, non-empty, and not identical to the packaged default templates.
- The assessment verifies each declared connector has the required permissions, provenance attestation, tenant/workspace scope, and prompt-injection boundary expected by the pack contract.
- The response is `ready` or `blocked`; it reports missing profiles, placeholder profiles, connector blockers, required profile/schema counts, and the onboarding workflow/eval ids.
- `workflow_pack.onboarding_assessed` audit evidence records the assessment result and blockers, but the assessment does not mutate install, stage, release, rollback, or archive state.

### Release

Release should require passing eval gates, policy gates, connector readiness, and approval where the pack declares high-risk handoffs or writes. Released pack behavior must be auditable and roll-backable.

The initial rollback contract is explicit and non-destructive:

- `POST /api/workflow-packs/installations/{id}/rollback` only accepts released installations.
- Rollback changes the installation status to `rolled_back` while preserving the original release timestamp, eval gate status, release gate status, manifest snapshot, and validation report.
- Rollback gate evidence is stored alongside the original release evidence under `gate_evidence.release` and `gate_evidence.rollback`.
- `workflow_pack.rolled_back` audit evidence records the rollback reason, timestamp, and gate evidence.
- A rolled-back installation can still be archived later; archive remains the step that removes it from active get/list APIs.

### Uninstall

Uninstall should archive the pack installation and disable future schedules or entrypoints. It must not delete historical artifacts, eval runs, audit logs, handoff events, or release evidence.

The initial Stage 3 lifecycle implements uninstall as a soft archive:

- `POST /api/workflow-packs/installations/{id}/archive` marks an installed, staged, released, or rolled-back installation as `archived`.
- Archived installations are excluded from active list and get APIs.
- The release timestamp, gate status, gate evidence, validation report, and manifest snapshot stay attached to the archived row for audit replay.
- `workflow_pack.archived` audit evidence records the archive reason, previous status, and archive timestamp.
- Archive is not a destructive package delete and does not erase historical audit or release evidence.

## Verification

Run:

```bash
scripts/verify-workflow-pack-manifest.sh
```

The gate currently runs Rust validator tests against the AI Governance Pack fixture and checks that the external JSON Schema and package manifest are present.

The Whiskey evidence gate exercises the API lifecycle end to end:

```bash
WORKFLOW_PACK_MANIFEST_PATH=packs/ai-governance/package.yaml scripts/workflow-pack-evidence-gate.sh
```

It validates the manifest, proves onboarding fails closed with placeholder/missing customer inputs, installs the pack, verifies release fails before staging, stages the installation, verifies release fails without passing gates, releases with passing eval/release gate evidence, rolls back the released installation, creates a new installed version from an updated manifest, proves onboarding reaches `ready` with customer-grounded profiles and connector declarations, verifies the rolled-back source remains unchanged and active before archive, archives the rolled-back source installation, and verifies archive removes only the source while the new installed version stays active.
