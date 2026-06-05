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
- DomainPack semantic scopes: `domain_scope`, `workflow_scope`, and `share_policy`.
- Profiles, skills, workflows, agents, connectors, schemas, policies, evals, and release gates.
- Onboarding workflow, required tenant profiles, profile schemas, and an onboarding eval.

All referenced files must be relative to the package directory, must exist, and must not use absolute paths or `..` escapes.

## Safety Rules

The validator enforces the initial Stage 3 safety floor:

- At least one workflow, agent, connector, required eval gate, and required release gate must exist.
- At least one profile, skill, schema, and policy must exist.
- The onboarding workflow must reference a declared workflow.
- DomainPacks must declare manifest-level semantic scopes with non-empty `domain_scope`, `workflow_scope`, and `share_policy`.
- DomainPack workflow files must also declare semantic scopes, including lane-specific scopes when the pack separates operational lanes.
- Onboarding required profiles must reference declared profile ids.
- Onboarding profile schemas must be relative, existing files.
- The onboarding eval must reference a declared eval.
- Connector writes must be approval-gated when enabled.
- Connectors must require provenance.
- Connectors must declare tenant and workspace scope as required.
- Connector outputs must be treated as data, not instructions.
- Connector data-quality contracts, when declared, must require at least one sample, a positive freshness window, and non-empty required metadata/content field lists.
- Reader agents cannot declare write or external-write tool scopes.
- Writer agents cannot declare external-write tool scopes.
- Handoffs must target declared agents.
- Handoffs must declare enum-like intents.
- Workflow file step references must resolve to declared agents, profiles, schemas, skills, and handoff intents.
- High-risk handoffs must require approval.
- Eval gate scores must be between `0` and `1`.

This contract does not install or activate packs yet. It blocks unsafe package shape before the later install/stage/release APIs exist.

## Lifecycle Semantics

### Install

Install should parse and validate the manifest, copy the package into a tenant-scoped immutable package store, and create a draft pack version. It must not create active agents, connectors, policies, schedules, or external writes by default.

The initial Stage 3 install flow also bootstraps customer-editable onboarding defaults:

- Install creates active version `1` onboarding profile assets for every required onboarding profile using the packaged template files.
- Those bootstrapped assets are intentionally placeholder defaults; onboarding assessment must still fail closed until customer-specific content replaces them.
- `workflow_pack.onboarding_defaults_bootstrapped` audit evidence records which onboarding profiles were seeded during install.

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

Stage should materialize draft workflow definitions, agent versions, connector definitions, policy revisions, eval suites, and profile requirements in a non-production state. Workflow bindings must carry the materialized `WorkflowDefinition` id as `target_id`, and workflow schedule runtime objects must reference that definition id. DomainPack semantic scopes must be preserved in workflow bindings, workflow schedules, workflow definitions, root task grants, and semantic-layer projections. Staging must preserve tenant scope and must not bypass provider, tool, approval, MCP, or audit governance.

### Onboarding Assessment

Customer onboarding must be explicit before a pack is treated as tenant-ready. The initial Stage 3 onboarding assessment API is contract-driven and non-destructive:

- `POST /api/workflow-packs/installations/{id}/onboarding/profiles` persists customer onboarding profiles as versioned assets for the installation. Saving a new profile revision archives the previous active revision for that profile.
- `GET /api/workflow-packs/installations/{id}/onboarding/profiles` lists the active persisted profile assets that will be reused by later onboarding checks.
- `POST /api/workflow-packs/installations/{id}/onboarding/assess` accepts active pack installations and evaluates customer-supplied onboarding profiles plus connector declarations against the installed manifest snapshot.
- Persisted onboarding profiles are used by default; inline profiles in the assessment request can override them for ad hoc checks.
- The assessment verifies required profiles are present, non-empty, and not identical to the packaged default templates.
- The assessment verifies each declared connector has the required permissions, provenance attestation, tenant/workspace scope, and prompt-injection boundary expected by the pack contract.
- The response is `ready` or `blocked`; it reports missing profiles, placeholder profiles, connector blockers, required profile/schema counts, and the onboarding workflow/eval ids, plus inline versus persisted profile counts.
- `workflow_pack.onboarding_profiles_saved` audit evidence records persisted profile revisions, and `workflow_pack.onboarding_assessed` records the readiness result and blockers. Neither endpoint mutates install, stage, release, rollback, or archive state.

### Connector Quality

Connector adoption must prove that retrieved data is fresh, attributable, and structurally usable before pack outputs rely on it.

- Connectors can declare an optional `data_quality` contract with `min_sample_count`, `max_age_hours`, `citation_required`, `required_metadata_fields`, and `required_content_fields`.
- `POST /api/workflow-packs/installations/{id}/connectors/quality/assess` evaluates connector sample payloads against that contract.
- The assessment checks sample freshness, citation presence, required metadata fields, required content fields, and minimum passing sample count.
- The response is `ready` or `blocked`; it reports per-connector sample counts, passing sample counts, and blocker lists.
- `workflow_pack.connector_quality_assessed` audit evidence records the connector-quality result and blockers, but does not mutate install, stage, release, rollback, or archive state.

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

The Tmall DomainPack fixture is also covered by the Rust validator so workflow-level semantic scope and reference contracts are exercised, not only manifest shape.

The Whiskey evidence gate exercises the API lifecycle end to end:

```bash
WORKFLOW_PACK_MANIFEST_PATH=packs/ai-governance/package.yaml scripts/workflow-pack-evidence-gate.sh
```

It validates the manifest, proves install bootstraps default onboarding profile assets, proves onboarding fails closed with placeholder/missing customer inputs, installs the pack, verifies release fails before staging, stages the installation, verifies release fails without passing gates, releases with passing eval/release gate evidence, rolls back the released installation, creates a new installed version from an updated manifest, proves the updated installation also boots default profile assets, persists customer onboarding profiles as reusable assets, proves onboarding reaches `ready` from persisted profile assets plus connector declarations, proves connector quality fails closed with stale/incomplete samples and reaches `ready` with fresh attributable samples, verifies the rolled-back source remains unchanged and active before archive, archives the rolled-back source installation, and verifies archive removes only the source while the new installed version stays active.
