# Managed Agents Console IA Design

## Summary

The MandoForge console should be designed around the core product abstraction:
managed agents. It should not become a generic business-process dashboard, and
it should not expose internal implementation modules as peer top-level pages.

The current top-level split between `Workflows`, `Dynamic`, and `Board` creates
three separate places to understand the same operational question: what work is
running, what is planned, what is blocked, and what needs human review. Those
views should be consolidated under one execution surface while preserving their
distinct semantics as subviews.

The target top-level IA is:

```text
Overview
Managed Agents
Runs & Tasks
Ontology
Capabilities
System Ops
```

Chinese-first labels:

```text
总览
托管智能体
运行与任务
本体与工具
能力包
系统运维
```

The default audience is a business user or enterprise operator who should be
able to understand the product without reading internal runtime concepts first.
Engineering details stay available, but behind drill-downs, advanced panels, or
secondary tabs.

## Goals

- Make `Managed Agents` the primary product object.
- Remove top-level duplication between `Workflow`, `Dynamic Workflow`, and
  `Board`.
- Keep Manager Agent as an observer/advisor, not a separate complex control
  system.
- Make Ontology visible as the semantic/tool layer that managed agents use.
- Make Capabilities visible as installable industry packs, connectors, and
  reusable agent capabilities.
- Make System Ops clearly mean platform operations, not business operations.
- Use Chinese-first labels with concise English secondary labels only where the
  product term is useful.

## Non-Goals

- Do not turn MandoForge into a generic business workflow SaaS.
- Do not remove workflow, dynamic planning, or task-board functionality.
- Do not make Manager Agent execute high-risk actions by default.
- Do not hide engineering/debug evidence completely; move it to advanced
  surfaces.
- Do not redesign backend APIs as part of this IA slice.

## Page Model

### 1. Overview / 总览

Purpose:

```text
Answer: Is the Agent OS healthy, what needs attention, and where should I go?
```

Primary content:

- Overall Agent OS health.
- Critical notifications.
- Active managed agents.
- Active runs and blocked tasks.
- Ontology readiness.
- Capability/connector readiness.
- System Ops readiness.
- Next actions.

Overview should not be a raw module dashboard. It should be a routing surface
and attention surface.

### 2. Managed Agents / 托管智能体

Purpose:

```text
Answer: Which managed agents exist, what are they doing, and where do they need
human help?
```

Primary content:

- Agent registry.
- Agent status: online, idle, running, blocked, failed.
- Sessions and current objectives.
- Tool calls.
- Approval requirements.
- Agent capability scope.
- Manager Agent observation rail.

Manager Agent placement:

- Right-side observation panel or top summary inside this page.
- It watches all agents, runs, tasks, approvals, and failures.
- It explains stuck/failing states and proposes next actions.
- It does not become a separate top-level product area.
- It does not execute high-risk writes without policy and approval.

Manager Agent examples:

```text
2 agents are idle.
1 run is waiting for approval.
3 tool calls failed repeatedly in the last 10 minutes.
Suggested next action: retry connector health check or escalate to operator.
```

### 3. Runs & Tasks / 运行与任务

Purpose:

```text
Answer: What work is running, planned, blocked, approved, or complete?
```

This page absorbs the current top-level `Workflows`, `Dynamic`, and `Board`
views.

Subview model:

```text
Runs & Tasks
  - Runs
  - Task Board
  - Workflow Templates
  - Dynamic Plans
  - Approvals
```

Definitions:

- `Runs`: execution history and active timelines across agent sessions,
  workflow runs, worker jobs, and task execution.
- `Task Board`: operational state buckets such as ready, running, review,
  blocked, backlog, and done.
- `Workflow Templates`: stable reusable definitions.
- `Dynamic Plans`: one-off DAG plans compiled from an objective by AI or
  Manager.
- `Approvals`: human decision queue for high-risk steps and low-confidence
  proposals.

Important naming decision:

- `Workflow` and `Dynamic Workflow` are not peer top-level concepts.
- A dynamic workflow is a dynamic plan under the broader execution surface.
- A workflow definition is a reusable template under the same surface.

### 4. Ontology / 本体与工具

Purpose:

```text
Answer: What does the enterprise data mean, and which governed tools can agents
use?
```

Primary content:

- Source tables and profiles.
- Object types.
- Properties and field mappings.
- Link types and relations.
- Metrics.
- Actions.
- Tool compiler output.
- Ontology PR review state.

Default view:

- Relationship graph or mind-map first.
- Raw JSON and detailed proposal lists belong in advanced/collapsed sections.

Conceptual relation:

```text
Enterprise data -> Ontology -> Agent tools -> Managed Agents
```

### 5. Capabilities / 能力包

Purpose:

```text
Answer: What capabilities can be installed or enabled for managed agents?
```

Primary content:

- Workflow packs.
- Domain packs.
- Connectors.
- Industry templates.
- Capability readiness.
- Connector production boundaries.
- Pack lifecycle state.

This page should not be only a marketplace list. It should make clear what each
capability enables for managed agents.

Examples:

- Ecommerce ontology pack.
- Tmall connector readiness.
- Taobao operations pack.
- Xianyu operations pack.
- Governance workflow pack.

### 6. System Ops / 系统运维

Purpose:

```text
Answer: Is the Managed Agent OS safe, deployed, auditable, and production-ready?
```

`System Ops` means platform operations, not business operations.

Primary content:

- Deployment version and environment.
- Desktop shell state.
- Tauri bridge / autostart / single-instance status.
- CSP, IPC, token, and permission boundary status.
- Audit logs.
- Usage and cost.
- Worker pressure.
- Alerts and incidents.
- Repair runbooks.
- Enterprise readiness evidence.

This page replaces the ambiguous meaning of `Operations`. The label should be
`System Ops` in English and `系统运维` in Chinese.

## Navigation Mapping From Current Views

Current:

```text
Overview
Wizard
Agents
Board
Workflows
Dynamic
Semantic
Packs
Deploy
Settings
```

Target:

```text
Overview       -> Overview / 总览
Wizard         -> Overview setup card or System Ops setup section
Agents         -> Managed Agents / 托管智能体
Board          -> Runs & Tasks > Task Board
Workflows      -> Runs & Tasks > Workflow Templates and Runs
Dynamic        -> Runs & Tasks > Dynamic Plans
Semantic       -> Ontology / 本体与工具
Packs          -> Capabilities / 能力包
Deploy         -> System Ops / 系统运维
Settings       -> System Ops > Settings or account/preferences drawer
```

The implementation can keep internal Rust/Yew component names while changing
the user-facing IA. Backend routes do not need to be renamed for this slice.

## Manager Agent Boundary

Manager Agent is an observer and advisor.

It may:

- Aggregate health across agents, runs, tasks, and approvals.
- Detect stuck work, repeated failures, idle capacity, missing approvals, and
  connector readiness blockers.
- Recommend next actions.
- Prepare draft repair plans.

It must not:

- Become a separate complex top-level control system.
- Hide the underlying evidence.
- Execute write-like or high-risk actions without explicit policy and approval.
- Replace operator judgment for ontology publication, connector writes, or
  production repair.

## Visual Hierarchy

Top-level pages should use the same hierarchy:

1. Page purpose statement.
2. Critical attention strip.
3. Primary work surface.
4. Manager/observer rail where relevant.
5. Evidence and advanced/debug sections.

For customer-facing default views:

- Show decisions, relationships, blockers, and next actions first.
- Show raw JSON, route names, and low-level IDs only behind details sections.
- Use Chinese-first text.
- Use English secondary labels only for durable product terms such as
  `Managed Agents`, `Runs`, `Ontology`, `Capabilities`, and `System Ops`.

## Acceptance Criteria

- The top-level navigation has six user-facing entries:
  `总览`, `托管智能体`, `运行与任务`, `本体与工具`, `能力包`, `系统运维`.
- There is no top-level `Dynamic` page.
- There is no top-level `Board` page.
- Workflow definitions, dynamic plans, task board, runs, and approvals are
  reachable under `Runs & Tasks`.
- Manager Agent appears as an observation/advice surface, not as a separate
  system.
- Ontology defaults to a relationship graph or mind-map before JSON/proposal
  details.
- System Ops content is clearly platform operations: deployment, desktop,
  security, audit, usage, health, and enterprise readiness.
- Existing backend APIs remain compatible.

## Risks and Mitigations

Risk: users still need fast access to engineering details.

Mitigation: keep advanced sections and deep links. The top-level IA changes
without removing underlying runtime evidence.

Risk: `Managed Agents` may be too English-heavy for Chinese-first UI.

Mitigation: display `托管智能体` as the primary label and `Managed Agents` as a
subtitle or page eyebrow.

Risk: consolidating pages creates a large `Runs & Tasks` page.

Mitigation: use internal tabs and default to the attention-oriented `Runs`
summary. Move dense tables to subviews.

Risk: current URL/localStorage behavior may still expose old view IDs.

Mitigation: keep old IDs as compatibility aliases while routing the visible nav
to the new IA.

## Verification Plan

- Rust/Yew compile check:
  `cargo check --manifest-path web-ui/Cargo.toml`
- Backend compile check after static CSP updates:
  `cargo check --manifest-path crates/mandoforge-api/Cargo.toml`
- Static asset build:
  `cd web-ui && env -u NO_COLOR trunk build --release`
- CSP hash sync for `web/index.html`.
- Browser verification with Chrome DevTools:
  - Open overview.
  - Confirm six top-level entries.
  - Open Managed Agents.
  - Open Runs & Tasks and confirm subviews.
  - Start ontology ecommerce demo and confirm graph-first layout.
  - Confirm console has no error/warn/issue messages.

## Open Implementation Notes

- This spec intentionally does not require immediate backend route changes.
- Existing generated static assets should be regenerated after Yew changes.
- `.superpowers/` is local brainstorming output and should not be committed.
