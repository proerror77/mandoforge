# Context OS Memory Architecture

This document defines how MandoForge should handle organizational memory for
managed agents, Workflow Packs, and domain digital-worker experiences.

The goal is not to create one shared agent brain. The goal is a governed Context
OS: scoped, source-attributed, replayable, and approval-gated memory that can be
used by many agents without leaking domain data across boundaries.

## Research Basis

This design is based on recurring patterns from current agent-memory systems and
research:

- Generative Agents separates memory stream, reflection, and planning. The
  important lesson is that agents need periodic synthesis, not just raw recall:
  <https://arxiv.org/abs/2304.03442>
- Reflexion shows that agents can improve from verbal self-reflection, but those
  reflections should be treated as generated evidence rather than trusted fact:
  <https://arxiv.org/abs/2303.11366>
- MemGPT / Letta popularized explicit memory tiers and context management
  instead of stuffing everything into one prompt:
  <https://arxiv.org/abs/2310.08560> and <https://docs.letta.com/>
- LangGraph memory guidance distinguishes thread-scoped and cross-thread memory,
  and separates semantic, episodic, and procedural memory:
  <https://langchain-ai.github.io/langgraph/concepts/memory/>
- Zep / Graphiti uses a temporal knowledge graph for agent memory. The useful
  pattern is relationship-aware memory with time and provenance, not vector
  search alone: <https://help.getzep.com/graphiti/>
- W3C PROV, RDF, and OWL provide the durable vocabulary for provenance, graph
  statements, and ontology expansion:
  <https://www.w3.org/TR/prov-overview/>,
  <https://www.w3.org/TR/rdf11-concepts/>, and
  <https://www.w3.org/TR/owl2-overview/>

The MandoForge conclusion: memory should be object/link/scope first, with
optional vector retrieval later. Reflection and dreaming should generate memory
candidates, not mutate durable organizational memory directly.

## Product Position

MandoForge is the Agent OS and middleware layer. Domain digital workers are
applications on top of it:

```text
MandoForge Context OS
  -> Social Media Workflow Pack
  -> E-commerce Ads Workflow Pack
  -> Legal Workflow Pack
  -> Finance Ops Workflow Pack
```

Each pack can use the same memory substrate, but each pack must receive only the
memory allowed by tenant, team, domain, workflow, case, data classification, and
human approval policy.

## Principles

- Default deny: no memory is shared across domains unless a policy explicitly
  allows it.
- Scope before retrieval: context selection starts from tenant, team, project,
  domain, workflow, agent, and case scopes.
- Source before summary: every durable memory needs source references and
  provenance.
- Reflection is not truth: post-run reflections and dreaming outputs become
  candidates until reviewed.
- Replayability is mandatory: operators must be able to inspect what context an
  agent saw before it acted.
- High-risk actions require trusted context: stale or unverified matching memory
  must block or downgrade high-risk execution.
- Ontology grows from objects already in use: start with lightweight object and
  relation types, then add domain ontologies where they reduce ambiguity.

## Memory Levels

MandoForge should treat memory as five levels.

| Level | Name | Sharing Default | Examples |
| --- | --- | --- | --- |
| L0 | Platform Memory | Shared across packs | Tool failure modes, retry behavior, prompt-injection rules |
| L1 | Tenant Common Memory | Shared within tenant by policy | Company profile, brand rules, approval matrix, org chart |
| L2 | Domain Memory | Isolated by default | Legal risk rules, social content strategy, ads optimization rules |
| L3 | Workflow Memory | Isolated to workflow | Contract review playbook, daily topic scan pattern, budget-change playbook |
| L4 | Case / Session Memory | Least shared | One contract, one campaign, one social post, one customer dispute |

The default sharing matrix:

| From / To | Social Media | E-commerce | Legal | Tenant Common |
| --- | --- | --- | --- | --- |
| Social Media | Allowed by scope | Denied | Denied | Selected summaries only |
| E-commerce | Denied | Allowed by scope | Denied | Selected summaries only |
| Legal | Denied | Denied | Allowed by scope | Selected policies only |
| Tenant Common | Allowed by policy | Allowed by policy | Allowed by policy | Allowed |

Social Media, E-commerce, and Legal memories should not share operational
details. They may share tenant-wide constraints such as brand safety, privacy
rules, external communication approval rules, and prohibited claims.

## Core Data Model

The existing Stage 5 substrate is the right foundation:

```text
semantic_sources
semantic_objects
semantic_links
context_packets
memory_writeback_candidates
```

The memory architecture should extend that substrate with stricter metadata and
policy semantics.

### Semantic Source

A source is where information came from.

Required fields:

```yaml
semantic_source:
  tenant_id: uuid
  source_type: repo_doc | session | artifact | mcp | feishu | github | upload | external
  source_uri: string
  owner_team_id: uuid?
  data_classification: public | internal | confidential | restricted
  freshness:
    state: unknown | current | stale | expired
    checked_at: timestamp?
  provenance:
    observed_at: timestamp
    observed_by: agent | human | connector | system
```

### Semantic Object

An object is a durable business fact, rule, decision, memory, artifact summary,
or workflow concept.

Required fields:

```yaml
semantic_object:
  tenant_id: uuid
  object_type: decision | runbook | policy | memory | artifact | business_object | metric | relation_rule
  object_key: string
  title: string
  summary: string
  content: object
  semantic_scopes:
    platform_scope: string?
    tenant_scope: string
    team_scope: string?
    project_scope: string?
    domain_scope: social_media | ecommerce | legal | finance | engineering | ops | common
    workflow_scope: string?
    agent_scope: string?
    case_scope: string?
    memory_scope: platform | tenant_common | domain | workflow | case
  data_classification: public | internal | confidential | restricted
  trust_level: unverified | source_attested | human_verified | system_verified
  freshness: unknown | current | stale | expired
  sharing_policy:
    default: deny | allow_same_scope
    allowed_domains: string[]
    allowed_workflow_packs: string[]
    allowed_agent_roles: string[]
    requires_human_approved_context: boolean
  source_refs:
    - semantic_source_id: uuid
      artifact_id: uuid?
      session_event_id: uuid?
      approval_id: uuid?
```

### Semantic Link

A link is a typed relationship between objects. This is the bridge toward a real
ontology.

Recommended relation types:

```text
applies_to
derived_from
supersedes
contradicts
requires_approval
owned_by
mentions
uses_metric
uses_tool
belongs_to_pack
belongs_to_workflow
is_example_of
```

The relation type should be allowlisted. Free-form relation text is useful for
notes, but not for policy decisions.

## Ontology Plan

The current MandoForge semantic layer is ontology-ready, not a full ontology
engine. The next step is to add a small core ontology plus domain ontologies.

### Core Ontology

Core objects shared across packs:

```text
Tenant
Team
Project
WorkflowPack
Workflow
Agent
Tool
Policy
ApprovalRule
Artifact
Metric
Decision
Memory
Case
```

Core relations:

```text
Agent executes Workflow
Workflow belongs_to WorkflowPack
Workflow requires Policy
Policy applies_to Tool
Memory derived_from Artifact
Decision approved_by Human
Artifact belongs_to Case
Metric measured_for BusinessObject
```

### Social Media Ontology

```text
Account
Platform
Post
Topic
ContentPillar
AudienceSegment
Campaign
EngagementMetric
BrandRisk
PublishingApproval
```

Typical relations:

```text
Post belongs_to Account
Post targets AudienceSegment
Topic belongs_to ContentPillar
Post measured_by EngagementMetric
BrandRisk requires PublishingApproval
```

### E-commerce Ontology

```text
Store
Product
SKU
Inventory
Campaign
Channel
AdSet
Creative
ROAS
Margin
BudgetRule
CustomerSegment
```

Typical relations:

```text
Campaign promotes Product
SKU has Inventory
Campaign measured_by ROAS
BudgetRule applies_to Campaign
Margin constrains BudgetRule
```

### Legal Ontology

```text
Contract
Party
Clause
Obligation
Risk
Jurisdiction
ApprovalRequirement
Template
NegotiationPosition
```

Typical relations:

```text
Contract contains Clause
Clause creates Obligation
Clause triggers Risk
Risk requires ApprovalRequirement
Template supersedes Clause
```

## Context Packet Builder

Agents should never pull memory directly from every source. The platform should
build a context packet before an agent acts.

```text
task intent
-> agent registry entry
-> runtime profile
-> workflow pack membership
-> semantic scopes
-> policy reminders
-> source freshness checks
-> semantic object retrieval
-> trust / sharing / classification gates
-> context packet
-> agent turn
```

The context packet should store:

```yaml
context_packet:
  session_id: uuid
  task_intent: object
  agent_id: uuid
  workflow_pack_ids: uuid[]
  semantic_scopes: object
  selected_object_ids: uuid[]
  excluded_object_refs:
    - object_id: uuid
      reason: scope_mismatch | sharing_denied | stale | untrusted | classification_denied
  policy_reminders: object[]
  freshness_warnings: object[]
  generated_at: timestamp
```

This makes context observable. If an agent makes a bad decision, the operator can
inspect whether the wrong memory was selected, the right memory was missing, or
the agent ignored valid context.

## Reflection And Dreaming

MandoForge needs two synthesis loops above ordinary task execution.

### Post-run Reflection

Triggered when a session, handoff, review, or workflow completes.

Input:

```text
session_events
tool_calls
artifacts
approvals
audit_logs
context_packets
human review notes
```

Output:

```text
memory_writeback_candidates
reflection_report artifact
possible semantic_links
```

The reflection should answer:

- What goal was attempted?
- What context was used?
- What worked?
- What failed or was corrected by a human?
- Which assumptions should not be reused?
- What durable memory candidate is justified by evidence?

### Dreaming Jobs

Dreaming is an offline synthesis job, not live task execution.

Suggested schedules:

| Domain | Schedule | Purpose |
| --- | --- | --- |
| Social Media | Daily and weekly | Identify content patterns, stale content pillars, platform drift |
| E-commerce | Hourly for anomalies, nightly for synthesis | Compare budget moves with results, detect rule drift |
| Legal | Weekly or monthly | Identify recurring clause risks and template update candidates |
| Platform | Daily | Detect tool failures, connector drift, policy friction |

Dreaming jobs may generate candidates, but cannot directly create durable
organizational memory.

```text
dreaming job
-> evidence bundle
-> proposed insight
-> confidence and counter-evidence
-> affected scopes
-> memory_writeback_candidate
-> human approve/reject
-> semantic_object(memory)
```

## Sharing And Isolation Policy

Memory selection should be enforced by the platform, not left to prompts.

Required gates:

```text
tenant gate
team/project membership gate
domain gate
workflow pack gate
agent role gate
case scope gate
data classification gate
trust gate
freshness gate
explicit sharing policy gate
```

Default rules:

- L0 Platform Memory may be shared across packs if it contains no tenant business
  data.
- L1 Tenant Common Memory may be shared across domains when allowed by tenant
  policy and role membership.
- L2 Domain Memory stays inside its domain by default.
- L3 Workflow Memory stays inside its workflow by default.
- L4 Case / Session Memory stays inside the case unless a human promotes a
  redacted summary.
- Cross-domain sharing requires an explicit policy and should prefer summaries
  over raw artifacts.

Example: Social Media should not see raw E-commerce margin data. It may see an
approved tenant-common rule such as "Do not publicly claim discount profitability
or guaranteed revenue impact."

Example: E-commerce should not see Legal contract details. It may see an
approved tenant-common rule such as "External vendor claims about AI automation
must be reviewed before publication."

## Writeback Lifecycle

Durable memory writeback must remain approval-gated.

```text
completed session / artifact / review / dreaming job
-> candidate generation
-> duplicate and contradiction check
-> scope proposal
-> reviewer approval
-> semantic_object(memory) created
-> semantic_links created
-> audit log and timeline event
```

Candidate metadata:

```yaml
memory_writeback_candidate:
  candidate_type: session_reflection | artifact_summary | human_review | dreaming_synthesis
  proposed_object_type: memory | policy | runbook | decision
  proposed_scopes: object
  proposed_sharing_policy: object
  evidence_refs: object[]
  confidence: low | medium | high
  counter_evidence: string?
  reviewer_subject: string?
  status: pending | approved | rejected
```

Approval should assign final scopes. The agent can propose scopes, but it should
not decide long-term sharing authority.

## Retrieval Strategy

Keep `scope_rank` as the default retrieval backend until the object/link policy
model is stable.

Recommended ranking order:

```text
1. Hard gates: tenant, membership, classification, sharing policy
2. Scope match: domain, workflow, agent role, case
3. Trust: human_verified > system_verified > source_attested > unverified
4. Freshness: current > unknown > stale > expired
5. Relationship distance: directly linked > one-hop > inferred
6. Recency and usage success
7. Optional vector score
```

Vector search can help recall, but it must not bypass gates. Treat vector
results as candidates that still pass object, link, provenance, trust, and scope
checks.

## Implementation Slices

### Slice 1: Documented Policy And Schemas

- Add memory level definitions to architecture docs.
- Extend semantic object schema with `domain_scope`, `memory_scope`,
  `data_classification`, and `sharing_policy`.
- Define allowlisted relation types.

### Slice 2: Context Packet Gate

- Record excluded candidate objects and exclusion reasons.
- Enforce explicit sharing policy when building context packets.
- Add timeline UI for selected and excluded context.

### Slice 3: Reflection Candidates

- Generate post-run reflection artifacts.
- Generate memory writeback candidates with evidence refs.
- Require human approval before durable writeback.

### Slice 4: Dreaming Jobs

- Add scheduled synthesis jobs per tenant/domain/workflow.
- Start with read-only reports and candidate generation.
- Keep automatic memory mutation disabled.

### Slice 5: Ontology Expansion

- Add core ontology object and relation types.
- Add first domain ontology for one pack, preferably Social Media or Legal.
- Add validation for typed relations.

### Slice 6: Optional Retrieval Backends

- Add vector or graph search only after gates are stable.
- Store retrieval backend scores as evidence, not as authority.

## Open Decisions

- Whether domain ontologies live in Workflow Packs, platform schemas, or both.
- Whether `memory_scope` should be a top-level column for fast filtering or stay
  inside `semantic_scopes`.
- How to represent redacted cross-domain summaries.
- Whether dreaming jobs require a separate review queue or reuse memory
  writeback review.
- Which pack should be the first ontology pilot: Social Media, E-commerce, or
  Legal.
