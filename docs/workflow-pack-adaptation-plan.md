# Workflow Pack Adaptation Plan

## Goal

Add `Workflow Pack` / `Domain Pack` as a first-class MandoForge product concept.

MandoForge should not copy a vertical legal AI product. The useful abstraction is an installable, versioned, auditable industry workflow package that runs on top of the Agent OS kernel.

The executable managed-workflow target is specified in
[Managed Agent Workflow Architecture](managed-agent-workflow-architecture.md).
This plan defines the pack concept; the architecture document defines how packs
materialize into WorkflowDefinition, WorkflowRun, TaskGrant, scoped worker
agents, runtime adapters, and workflow observability.

The runtime stays generic:

```text
Agent OS Core
  -> profiles
  -> skills
  -> workflows
  -> connectors
  -> schemas
  -> policies
  -> evals
  -> approvals
  -> audit / replay
```

Domain packages provide the business-specific layer:

```text
Workflow Packs
  -> AI Governance Pack
  -> Privacy Pack
  -> Commercial Contract Pack
  -> Regulatory Intelligence Pack
  -> Engineering Pack
  -> Finance Ops Pack
  -> E-commerce Ops Pack
```

## External Pattern Being Adapted

Anthropic's plugin ecosystems are useful as reference architecture, not as something to clone directly.

Observed reusable patterns:

- A plugin is a self-contained bundle with manifest, skills, agents, connectors, and docs.
- A skill is a versionable task unit with instructions and supporting assets.
- A domain profile captures the team's house style, risk posture, and operating rules.
- MCP connectors bring enterprise systems into the workflow, but connector outputs must be treated as data, not instructions.
- Managed-agent cookbooks separate orchestrators from leaf workers and route work through typed handoffs.
- Regulated or high-impact outputs stay draft-only until a qualified human reviews them.

This maps cleanly to MandoForge's Agent OS direction: workflow behavior should be portable across UI, model provider, and execution substrate as long as the runtime can enforce policy, approval, audit, and replay.

## New Product Concept

### Workflow Pack

A Workflow Pack is an installable, versioned, auditable industry workflow package.

It contains:

- Skills: repeatable task definitions.
- Agents: worker roles, scheduled monitors, and event-driven processors.
- Workflows: ordered or event-driven execution graphs.
- Profiles: tenant, company, department, and domain playbooks.
- Connectors: MCP or native connector manifests.
- Schemas: typed inputs, outputs, handoffs, extraction records, and artifact contracts.
- Policies: tool scopes, approval rules, escalation rules, and risk tiers.
- Evals: golden cases and regression gates.
- Docs: operator notes, setup guide, and non-goals.

### Proposed Package Layout

```text
agent-os-pack/
  package.yaml
  profiles/
    default.md
    onboarding.md
  skills/
    use-case-triage/skill.md
    vendor-ai-review/skill.md
    policy-monitor/skill.md
  workflows/
    renewal-watcher.workflow.yaml
    reg-monitor.workflow.yaml
  agents/
    reader.agent.yaml
    analyzer.agent.yaml
    writer.agent.yaml
  connectors/
    mcp.yaml
  schemas/
    extracted_contract.schema.json
    handoff.schema.json
  evals/
    golden_cases.jsonl
  policies/
    approval.yaml
    tool_scope.yaml
```

## Cold-Start Onboarding

Workflow Packs should not ask customers to manually fill a large static config first.

Add an onboarding workflow that turns customer knowledge into executable profiles:

```text
tenant_profiles/
  company.md
  legal.md
  sales.md
  engineering.md
  data_governance.md
  approval_matrix.yaml
  connector_map.yaml
  output_style.md
  risk_policy.yaml
```

The onboarding agent should gather:

- Company context.
- Department workflow playbook.
- Approval matrix.
- Risk policy.
- Data source map.
- Connector availability.
- Output style and template preferences.
- Reference documents or prior accepted outputs.

This is the lightweight enterprise semantic layer for a pack. Without it, a workflow pack will produce generic output.

## Worker Safety Pattern

Do not let one all-powerful agent read untrusted documents, reason over business policy, execute tools, write outputs, send external messages, and make final decisions.

Use a tiered worker pattern:

```text
Reader Agent
  -> reads untrusted source material
  -> emits schema-validated JSON only

Analyzer Agent
  -> sees structured JSON plus policy/profile
  -> emits risk, findings, options, and action plan

Writer Agent
  -> creates draft output
  -> does not directly execute consequential actions

Approval Console
  -> human review, modify, approve, reject

Executor Tool
  -> performs external write only after approval
```

Runtime requirements:

- Reader has read-only connector/file access.
- Analyzer does not receive raw untrusted documents when a structured extraction is sufficient.
- Writer cannot perform external writes by itself.
- Consequential actions require approval.
- All handoffs and tool calls are audit logged.

## Typed Handoff Events

Agent-to-agent routing should not be free-form natural language.

Add a formal handoff event model:

```text
agent_handoff_events
  id
  source_session_id
  source_agent
  target_agent
  intent
  payload_json
  schema_version
  risk_level
  approval_required
  status
  audit_trace_id
```

Example:

```json
{
  "type": "handoff_request",
  "target_agent": "privacy-reviewer",
  "intent": "run_pia",
  "params": {
    "feature_id": "LAUNCH-123",
    "jurisdiction": ["EU", "US-CA"],
    "data_categories": ["behavioral", "email"]
  }
}
```

Runtime rules:

- `intent` must come from a fixed enum.
- `target_agent` must be allowlisted for the source agent or workflow.
- `params` must validate against a JSON schema.
- Free text must be wrapped as data, not steering instructions.
- Handoff events must be persisted and replayable.
- Approval should be required for high-risk handoffs.

## Connector Rules

Connector manifests should make trust boundaries explicit:

```yaml
connector:
  name: ironclad
  type: mcp
  auth: oauth
  permissions:
    - contract.search
    - contract.read
  writes:
    enabled: false
  provenance:
    required: true
  prompt_injection_boundary:
    treat_results_as_data: true
  tenant_scope:
    tenant_id: required
    workspace_id: required
```

Rules:

- Default connectors should be read-heavy.
- Writes must be disabled or approval-gated by default.
- Connector results require provenance: source, retrieval time, object id, citation-ready reference when possible.
- Connector content must be treated as untrusted data.
- Errors, missing permissions, and rate limits should degrade gracefully.
- Tenant and workspace scope must be explicit.

Initial connector targets for Asia / China-facing customers:

- Feishu / Lark.
- WeCom / Enterprise WeChat.
- DingTalk.
- Tencent Docs / Kingsoft Docs.
- Jira / Linear / GitHub.
- Google Drive / Slack.
- CLM / DMS / contract repositories.
- Internal knowledge bases.
- Finance systems.
- HRIS.
- Legal matter systems.

## First Workflow Packs

Do not start by building a complete legal suite. Start with packs that match the Agent OS wedge: enterprise AI adoption, governance, and compliance.

| Priority | Workflow | Pack | Why |
| --- | --- | --- | --- |
| P0 | AI Use Case Triage | AI Governance Pack | First gate for enterprise AI adoption. |
| P0 | AI Impact Assessment | AI Governance Pack | Strong consulting and compliance deliverable. |
| P1 | Vendor AI Review | AI Governance + Commercial Contract Pack | Common need when customers buy AI tools. |
| P1 | PIA / DPA Review | Privacy Pack | Useful for SaaS, cross-border, e-commerce, finance, and insurance customers. |
| P2 | Regulatory Monitor | Regulatory Intelligence Pack | Good recurring subscription workflow. |

## Implementation Plan

### Slice 1: Pack Contract

- Define `WorkflowPack` metadata and package layout through `schemas/workflow-pack-manifest.schema.json` and [WorkflowPack Manifest Contract](workflow-pack-manifest-contract.md).
- Add manifest schema for pack id, version, capabilities, required connectors, policies, evals, profiles, worker roles, handoff rules, and release gates.
- Add Rust validation plus a test fixture for package structure in `crates/mandoforge-api/src/workflow_pack.rs` and `packs/ai-governance/package.yaml`.
- Validate the contract with `scripts/verify-workflow-pack-manifest.sh`.
- Document install/update/uninstall semantics in [WorkflowPack Manifest Contract](workflow-pack-manifest-contract.md).

### Slice 2: Profile Onboarding

- Add cold-start onboarding workflow contract through the manifest `onboarding` section.
- Persist tenant/domain profiles as versioned artifacts in the pack profile contract.
- Add approval matrix, connector map, risk policy, output style, company, and department schemas.
- Add eval cases for "generic output before profile" versus "profile-grounded output after onboarding".

### Slice 3: Handoff Events

- Add `agent_handoff_events` storage.
- Add schema-validated handoff API.
- Add allowlist checks between source agent, target agent, workflow, and intent.
- Add audit log and timeline events for request, accept, reject, fail, and complete.

### Slice 4: Reader / Analyzer / Writer Safety

- Add pack-level worker role declarations.
- Enforce tool scopes per role.
- Add tests proving Reader cannot write, Writer cannot read untrusted raw source unless explicitly allowed, and Analyzer only receives schema-validated data.
- Add UI evidence for handoff chain and approval status.

### Slice 5: First AI Governance Pack

- Implement:
  - AI use case triage.
  - AI impact assessment.
  - Vendor AI review.
  - Policy monitor.
- Keep all legal/compliance outputs as drafts.
- Require provenance for source-backed claims.
- Require approval for external writes or final policy publication.

## Product Principles

- Workflow packs are portable; they should not be tied to one UI or one model provider.
- Profiles are customer-specific assets and should be versioned.
- Connectors return data, not instructions.
- Consequential writes require approval.
- Regulated outputs are drafts until a qualified human approves them.
- Handoffs are typed, allowlisted, schema-validated, and audited.
- Evals ship with packs, because pack behavior must not regress silently.

## Open Questions

- Should pack installation create agent versions immediately, or stage them as draft releases first?
- Should profiles live in the artifact store, a dedicated profile store, or both?
- How much of pack validation belongs in Rust API versus a developer CLI?
- Should Workflow Packs be tenant-local packages first, or should the repo also define a marketplace/catalog model?
