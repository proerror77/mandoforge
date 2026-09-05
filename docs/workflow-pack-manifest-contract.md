# WorkflowPack Manifest Contract

This is the Stage 3 contract for installable Workflow Packs and Domain Packs.

The first implementation is intentionally contract-first: it validates package shape, file references, connector trust boundaries, worker roles, handoff rules, eval gates, and release gates before any runtime install API is allowed to mutate tenant behavior.

## Agent Harness Alignment

This contract follows the provider-neutral harness rule from
`agents-best-practices`: models and pack authors may propose actions, workflow
plans, connector bindings, and domain policies, but the MandoForge runtime owns
validation, authorization, execution, audit, and observation.

For Workflow Packs, that means:

- A pack manifest is not runtime authority by itself. It is a versioned package
  proposal that must pass manifest validation, policy gates, eval gates,
  connector readiness, and release approval before tenant behavior changes.
- Workflow plans and handoffs must be durable runtime objects, not hidden prompt
  state. They need declared scope, allowed tools, risk class, verification
  strategy, budget, and replayable evidence.
- Connectors and retrieved data are treated as data, not instructions. They must
  carry provenance, tenant/workspace scope, prompt-injection boundaries, and
  quality evidence before agent outputs can rely on them.
- Draft and commit stay separate for risky behavior. External writes,
  privileged actions, destructive changes, financial operations, and high-risk
  handoffs require approval or explicit policy gates outside the model.

Reference: <https://github.com/DenisSergeevitch/agents-best-practices>

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

All referenced files must be relative to the package directory, must exist, and must not use absolute paths, `..` escapes, or symlinks whose resolved target is outside that directory.

## Safety Rules

The validator enforces the initial Stage 3 safety floor:

- At least one workflow, agent, connector, required eval gate, and required release gate must exist.
- At least one profile, skill, schema, and policy must exist.
- The onboarding workflow must reference a declared workflow.
- DomainPacks must declare manifest-level semantic scopes with non-empty `domain_scope`, `workflow_scope`, and `share_policy`.
- DomainPack workflow files must also declare semantic scopes, including lane-specific scopes when the pack separates operational lanes.
- DomainPack workflow files must declare workflow-level observability with expected events, required evidence, at least one positive budget limit, and failure-report fields.
- DomainPack workflow steps must declare step-level observability with a unique `step_key`, expected events, and required evidence.
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
- Agents that expose `native.connector.call` may narrow their authority with
  `native_connector_actions`, whose values must reference declared action ids.
  A pack with more than one native connector Agent must declare this list for
  every such Agent; ambiguous shared action authority is rejected.
- Handoffs must target declared agents.
- Handoffs must declare enum-like intents.
- Workflow file step references must resolve to declared agents, profiles, schemas, skills, and handoff intents.
- High-risk handoffs must require approval.
- Child handoff grants intersect the parent connector scope with the target
  AgentVersion's exact connector, operation, and side-effect bindings. Missing
  target bindings fail closed instead of inheriting the root grant wholesale.
- Eval gate scores must be between `0` and `1`.

Manifest validation checks package structure. Installation, staging, and release
are separate API operations with their own state and authority checks.

### Skill instructions and runtime evidence

Business skills use manifest IDs and paths; developer-skill YAML frontmatter is
not required. A workflow step selects skills through its `skills` array. Staging
revalidates the current package and loads the sorted, deduplicated union of the
skills selected by each agent's steps across that pack. An agent with no selected
skills keeps its original instructions; declaring a file alone does not select it.

Skill files must be nonempty UTF-8 and at most 64 KiB each. The selected skill
content for one agent is limited to 256 KiB; excess content fails staging rather
than being truncated. Missing, malformed, or escaped sources and invalid step
references also block staging before prepared agents are persisted.

The materialized AgentVersion stores the loaded text in `system_prompt`, the IDs
in `skill_ids`, and provenance in `runtime_config.workflow_pack.skills` (ID,
relative source path, and `source_digest`). The digest uses the existing pack
source convention: SHA-256 of the normalized JSON string containing the source
text. Installation ID and pack version accompany it in `runtime_config.workflow_pack`.
Provider harness input and delegated CLI/App Server messages use the session's
pinned AgentVersion instructions. Tools, TaskGrant, tenant isolation, and
approval policy remain authoritative; skill text adds no permissions.

Changing a source file does not update already materialized versions or existing
sessions. To adopt edits, use the pack's update/stage/release lifecycle to create
new materialized versions. Legacy versions created without skill instructions
remain unchanged; upgrading the API alone does not backfill or activate them.

`scripts/verify-workflow-pack-manifest.sh` includes deterministic regressions for
loading, session pinning, and rejection boundaries as well as structural checks.
Neither those checks nor the presence of `golden_cases.jsonl` establishes live
model quality or production readiness. Quality claims require an executed eval
with its model/version, inputs, outputs, assertions, and gate evidence; connector
production claims still require the corresponding live readback.

## Lifecycle Semantics

### Install

Install parses and validates the manifest and records an installation with pending
gates. It snapshots manifest data and onboarding defaults while retaining the
source package path; it does not currently copy the whole package into an
immutable source store. Staging revalidates sources and pins the selected skill
instructions to new AgentVersions. Installation alone does not activate agents,
connectors, schedules, or external writes.

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
