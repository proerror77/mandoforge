# Managed Agent Workflow Architecture

This document defines the target architecture for MandoForge as a Managed Agent
Workflow Runtime. It is based on the current repository state and the reference
patterns in `anthropics/claude-for-legal` at commit
`b0aeeba7fea60a5de911549f2852fa5b078c7d76`.

## Decision Summary

MandoForge should not be modeled as one digital employee, one vertical agent, or
one tool-calling demo. The durable product model is:

```text
WorkflowPack / DomainPack
  -> versioned agents, skills, workflows, schemas, connectors, policies, evals
  -> materialized WorkflowDefinition / AgentVersion / ConnectorBinding
  -> WorkflowRun / WorkflowStepRun
  -> TaskGrant + MemoryScope + ToolScope + ConnectorScope
  -> Session / SessionThread / AgentHandoffEvent
  -> Environment-bound runtime adapter
  -> Worker lease executes the skilled worker agent
  -> events, artifacts, approvals, audit, memory writeback candidates
```

The most important correction is the boundary:

- Runtime / Orchestrator coordinates work. It does not perform business actions.
- Worker is a runner or lease owner, not the authority model.
- AgentVersion is behavior and prompt configuration.
- SkillPack is capability and procedure.
- TaskGrant is the task-level authority boundary.
- Environment decides where the agent runs.
- Runtime adapter executes the agent loop through Codex App Server, Codex CLI,
  Claude Code, or another backend.

`codex.exec` and `agent_cli.exec` may remain compatibility facades, but product
semantics should be `workflow.run.created`, `workflow.step.assigned`,
`agent.handoff.requested`, `agent.thread.created`, `task_grant.issued`,
`runtime.turn.completed`, and `task.result.received`.

## Reference Pattern From `claude-for-legal`

The useful pattern is not the legal content. The useful pattern is the split
between domain assets and managed-agent deployment shape:

- One source of truth can power interactive plugins and headless managed agents.
- Skills contain task procedures and supporting assets.
- Agent manifests declare role, prompt, tools, MCP servers, and callable agents.
- Managed-agent cookbooks are thin deployment adapters, not a second business
  logic layer.
- Orchestrators route work; leaf agents do the domain work.
- Reader, analyzer, and writer roles reduce blast radius.
- Cross-agent handoff is allowlisted, typed, schema checked, and audited.
- Connector output is data, not instruction.

MandoForge should absorb those boundaries but not copy the prompt-first
`steering-examples.json` control plane. MandoForge already has a stronger
native contract: `package.yaml`, workflow files, agent manifests, typed
handoffs, sessions, threads, events, policy, approval, artifacts, and audit.

## Current MandoForge Baseline

The repository already has the managed runtime kernel:

- Agent registry and AgentVersion.
- Environment records for local, cloud, self-hosted, remote computer, and Codex
  App Server placement.
- Session, ordered session events, streaming, and replay.
- SessionLoopJob queue with lease and event cursor windows.
- SessionThread for primary and handoff threads.
- AgentHandoffEvent and AgentHandoffAssignment.
- ManagerAgentPlan.
- Tool Router, PolicyConfig, Approval, ToolCall, Artifact, AuditLog.
- Runtime adapters for Codex CLI, Codex App Server, and managed agent CLI.
- WorkflowPack manifest validation, install, onboarding, connector quality,
  stage, release, rollback, and archive lifecycle.
- WorkflowDefinition, WorkflowRun, and WorkflowStepRun persistence and APIs.
- WorkflowRun start now materializes the first `step_graph.steps` entry, or all
  entries marked `start: true` / `entrypoint: true`, into queued
  WorkflowStepRun records bound to the root TaskGrant.
- WorkflowStepRun updates now advance dependency-based `step_graph` execution:
  completed steps materialize downstream steps whose `depends_on` / `after` /
  `needs` dependencies are satisfied, and the WorkflowRun reaches `completed`
  after all graph steps are successful.
- TaskGrant persistence and APIs, root grant issuance on workflow start, child
  grant narrowing checks, and checked/denied audit events.
- Handoff assignment now materializes specialist WorkflowStepRun records and
  narrowed child TaskGrants linked by `handoff_id`, `source_handoff_id`, and
  `workflow_step_run_id`.
- SemanticSource, SemanticObject, SemanticLink, ContextPacket, and
  MemoryWritebackCandidate.
- Workflow primary-session tool execution and context-packet generation consume
  active TaskGrants before policy, tool, connector, or memory access.
- MemoryScope now gates context object type/id/max count, trust threshold, and
  writeback permission.
- ApprovalCommitToken is implemented for `mcp.call`, `native.connector.call`,
  and external-write scoped commit tools when `ConnectorScope.mode =
  commit_write`: approval binds the normalized args hash and target binding, and
  worker execution revalidates the active grant, connector scope, secret refs
  where applicable, and the token before execution.
- Durable `WorkflowTransition` records are written for start-step
  materialization, dependency advancement, and workflow completion.
- Workflow Pack stage now materializes durable `workflow_pack_bindings` for
  manifest objects such as workflows, agents, connectors, policies, evals,
  schemas, skills, profiles, onboarding schemas, and release gates; release,
  rollback, and archive update their binding status.
- Native connector commit writes now enforce connector id, operation,
  side-effect class, external effect flag, and exact approval digest binding
  before the approved execution path can mark the call complete.
- The web console now observes workflow runs, steps, transitions, task grants,
  workers, approvals, tool calls, artifacts, and session events from live APIs.

The main gap is not "can it run a tool". The main gap is "can an installed
Workflow Pack become an executable, governed, observable workflow graph".

Remaining first-class execution gaps:

- Workflow graph scheduling is still dependency-based. It has durable transition
  records, but not full branch/skip/fail/retry/compensation policy execution.
- Pack bindings are materialized as generic durable binding records. Native
  runtime objects for schedules, connector accounts, and provider-specific
  deployment handles still need dedicated adapters.
- Native connector enforcement is implemented at the generic
  `native.connector.call` boundary. Production connector adapters still need
  service-specific target validation, rate limits, reconciliation, and rollback
  semantics.
- Workflow observability is now present in the web console, but richer graph
  visualization, transition filtering, and pack-binding inspection remain UI
  follow-ups.

Current boundary status:

- Clear and enforced now: workflow run persistence, root TaskGrant issuance,
  child-grant non-expansion, workflow-session grant requirement, tool-scope
  checks, MCP connector allowlist checks, context-packet memory object
  filtering, MemoryScope writeback/trust gates, handoff-to-step/grant
  materialization, `step_graph` start-step materialization plus dependency
  advancement, durable transition recording, pack binding materialization, and
  `ApprovalCommitToken` exact binding for MCP and native connector commit
  writes.
- Clear but only partially enforced now: worker/agent class authority,
  branch/skip/fail/retry/compensation transition policies, production native
  connector transport semantics, provider-specific pack binding deployment, and
  advanced workflow observability UI.

## Product Object Model

### WorkflowPack / DomainPack

A pack is an installable industry workflow package. It is application layer, not
the Agent OS itself.

It contains:

- `profiles`: company, department, domain, approval matrix, connector map,
  output style, and risk policy.
- `skills`: reusable task methods and assets.
- `agents`: orchestrator, reader, analyzer, writer, executor roles.
- `workflows`: typed workflow graphs and entrypoints.
- `connectors`: MCP or native connector requirements.
- `schemas`: input, output, handoff, artifact, and extraction contracts.
- `policies`: tool, memory, connector, approval, and release policies.
- `evals`: golden cases and release gates.

Install stores the package. Stage materializes draft runtime objects. Release
activates those objects for tenant use. Rollback disables future entrypoints but
preserves history.

### WorkflowDefinition

`WorkflowDefinition` is the normalized runtime version of a pack workflow file.

It should include:

- `id`, `tenant_id`, `pack_installation_id`, `pack_version`
- `name`, `entrypoint`, `trigger_type`
- `input_schema_ref`, `output_schema_ref`
- `default_agent_id`, `default_environment_id`
- `step_graph`
- `handoff_rules`
- `approval_policy_ref`
- `eval_gate_refs`
- `release_state`

This gives UI and runtime a stable object to start and inspect workflows without
re-parsing raw pack files on every run.

### WorkflowRun

`WorkflowRun` is one business execution of a workflow.

It should include:

- `id`, `tenant_id`, `workflow_definition_id`, `pack_installation_id`
- `source_event_id`, `source_work_item_id`, `source_schedule_id`
- `status`: `queued | running | requires_action | completed | failed |
  canceled`
- `primary_session_id`
- `root_task_grant_id`
- `input_payload`, `input_digest`
- `started_at`, `completed_at`
- `audit_trace_id`

`WorkflowRun` is the UI anchor. Operators should not have to infer a workflow
from unrelated session logs.

### WorkflowStepRun

`WorkflowStepRun` tracks each planned or emergent step.

It should include:

- `workflow_run_id`
- `step_key`, `step_type`
- `agent_id`, `agent_version_id`
- `session_id`, `thread_id`, `handoff_id`
- `task_grant_id`
- `environment_id`
- `status`
- `input_payload`, `output_payload`
- `artifact_ids`, `approval_ids`, `tool_call_ids`
- `started_at`, `completed_at`

Step runs may be sequential or parallel. The orchestrator decides the graph
within the limits of the WorkflowDefinition and TaskGrant.

### AgentVersion

AgentVersion is the behavior snapshot:

- model/provider/runtime profile
- system prompt
- allowed tools
- skill ids
- MCP server ids
- approval policy
- runtime config
- semantic scopes
- workflow pack ids

It should not be treated as the full permission boundary. It is an upstream
template for TaskGrant.

### TaskGrant

TaskGrant is the non-negotiable security primitive for managed workflows. It is
issued per workflow run, handoff, step, or execution job. It is immutable except
for status changes such as revoke, expire, consume.

Required fields:

```text
TaskGrant
  id
  tenant_id
  workflow_run_id
  workflow_step_run_id
  session_id
  parent_grant_id
  source_event_id
  source_handoff_id
  issuer_subject
  grantee_agent_id
  grantee_session_id
  agent_class
  objective
  risk_level
  status
  expires_at
  max_turns
  max_tool_calls
  max_runtime_seconds
  max_cost_usd_micros
  semantic_scopes
  memory_scope
  tool_scope
  connector_scope
  approval_policy
  external_effects
  context_packet_id
  policy_revision_id
  immutable_args_hash
  audit_trace_id
```

Rules:

- A child grant can only narrow the parent grant.
- Approval cannot expand a grant.
- Worker must revalidate the grant before execution.
- Tool results, artifacts, and memory candidates must not exceed grant scope.
- A grant expires or is consumed; it is not a standing agent permission.

### MemoryScope

`semantic_scopes` are not enough. They are labels for matching context. They do
not by themselves answer what this task may read.

MemoryScope should define:

- `mode`: `snapshot_only | scoped_lookup`
- allowed scope keys
- allowed object types
- allowed source types
- allowed object ids
- minimum trust level
- freshness limit
- max objects
- whether approval memory is allowed
- whether handoff memory is allowed
- whether memory writeback candidates are allowed

Default should be `snapshot_only`: the worker receives a ContextPacket snapshot
and cannot query the broader semantic store directly.

Memory sharing rule:

- Company-wide policies, approved operating rules, public templates, taxonomies,
  and human-verified profile facts can be shared if the grant declares them.
- Social media, ecommerce, legal, finance, HR, and customer-specific workflows
  should not share task memory by default.
- Cross-pack memory requires an explicit shared scope such as
  `company_profile`, `approved_policy`, `brand_style`, or `shared_taxonomy`.
- Legal pack memory should be isolated by matter, client, jurisdiction, and
  privilege scope.
- Social media pack memory should be isolated by brand, channel, campaign, and
  account.
- Ecommerce pack memory should be isolated by store, marketplace, product line,
  ad account, and region.

### ConnectorScope

`mcp.call` is a transport. It is not the authorization object.

ConnectorScope should define:

- allowed connector/server ids
- allowed connector tool names
- mode: `read_only | draft_write | commit_write`
- tenant/workspace scope
- allowed recipients, channels, accounts, endpoints, regions
- allowed side effect class
- required provenance
- secret reference ids

External effects are denied by default:

```text
publish = false
payment = false
external_message = false
account_mutation = false
ad_spend_mutation = false
```

Publishing, payment, ad spend mutation, or external messages require:

- TaskGrant allows that external effect.
- ConnectorScope mode is `commit_write`.
- ApprovalCommitToken exists.
- The token binds args hash, target, recipient/channel/account, amount or spend
  limit, and content digest.
- Worker revalidates the grant and token immediately before execution.

Current runtime enforcement covers `mcp.call`, `native.connector.call`, and
external-write scoped commit tools. The generic commit binding includes the
full call args hash, payload digest, target fields, optional
side-effect-class/account/channel/recipient/amount/spend-limit fields, and a
content digest when content-like payload fields are present. It rechecks active
grant, connector scope, and ApprovalCommitToken at worker time. MCP execution
also rechecks team MCP server/tool scope and runtime secret refs before the
gateway call. Native connector adapters still need service-specific transport,
amount/spend reconciliation, and rollback semantics.

### ApprovalCommitToken

Normal Approval means a human reviewed a blocking request. For irreversible
external effects, approval must bind the exact side effect.

Current implementation covers MCP and native connector commit calls with
`ConnectorScope.mode = commit_write`. The token binds:

- `grant_id`
- `tool_name`
- `normalized_args_hash`
- `connector_target`
- `recipient/channel/account`
- optional `side_effect_class`, amount, spend-limit, context packet, payload
  digest, and content digest fields
- `expires_at`
- `approver_subject`

Changing target, recipient, platform, account, side-effect class, payload, or
call args after approval invalidates the token. Production native connector
adapters should add explicit service-specific validation and reconciliation
where amount, spend, content digest, campaign, recipient, or account concepts
are first-class.

## Runtime Flow

### 1. Trigger

The trigger can be user input, schedule, webhook, WorkItem, connector event, or
API call.

Runtime creates:

- `workflow.run.created`
- `WorkflowRun`
- primary `Session`
- primary `SessionThread`
- root `TaskGrant`
- initial `ContextPacket`

### 2. Orchestration

The orchestrator agent receives:

- workflow objective
- allowed workflow definition
- root grant
- context packet snapshot
- available specialist agents
- handoff schema and allowlist

It may:

- create a plan
- create step runs
- request typed handoffs
- request approvals
- create artifacts
- finish or escalate

It may not:

- call business connectors directly for consequential action
- publish, email, pay, change ad spend, or mutate external accounts
- execute shell/Codex/browser actions unless given an explicit executor grant
- expand memory/tool/connector scope

### 3. Handoff

For specialist work, the orchestrator creates a typed handoff:

```text
agent.handoff.requested
  target_agent
  intent
  payload
  schema_version
  risk_level
  approval_required
  narrowed semantic_scopes
  narrowed TaskGrant
```

The runtime validates:

- target agent exists and is allowed
- target role matches workflow definition
- intent is enum-like and allowed
- payload validates against schema
- risk and approval rules are satisfied
- child grant is a subset of parent grant

Then it creates:

- `WorkflowStepRun`
- child `SessionThread`
- optional specialist `Session`
- `task_grant.issued`
- `agent.handoff.assigned`

### 4. Worker Agent Execution

The worker is a runner:

```text
Worker lease
  -> claim session-loop job or execution job
  -> load Session / Environment / AgentVersion / TaskGrant
  -> revalidate grant
  -> call runtime adapter
  -> ingest normalized runtime events
  -> persist tool calls, approvals, artifacts, audit
  -> release or complete lease
```

The skilled worker agent is the combination of:

- AgentVersion
- skills
- context packet
- TaskGrant
- Environment
- runtime adapter

The runtime adapter may use Codex App Server, Codex CLI, Claude Code, CDP, MCP,
or another substrate. The action still flows through Tool Router, grant checks,
policy, approval, and audit.

### 5. Result And Memory

Specialist results return as:

- structured `task.result.received`
- artifacts
- step output payload
- handoff completion event
- summary in parent thread
- optional MemoryWritebackCandidate

Memory writeback is never automatic high-trust memory. It should be:

- candidate first
- scoped
- provenance-linked
- reviewed or eval-gated
- promoted only after approval

Reflection or "dreaming" should be implemented as a scheduled workflow, not as
an unbounded background process. It can read completed runs, propose memory
candidates, summarize lessons, and create eval cases, but it cannot silently
promote memory or change policies.

## Agent Role Permission Matrix

| Role | Default allowed | Default denied |
| --- | --- | --- |
| Orchestrator | Create plan, issue narrowed child grant, create handoff, request approval, create artifact, read approved context packet | Shell/Codex execution, file write, connector commit, publish, payment, external message, memory promotion |
| Reader | Read files/connectors allowed by grant, produce schema-validated JSON | Writes, execution, external effects, approval decision |
| Analyzer | Consume structured reader output, policy/profile context, create findings and draft plan | Raw untrusted source when not needed, writes, execution, connector commit |
| Writer | Create draft artifacts and scoped file writes when explicitly granted | Connector calls, shell/Codex execution, publish, payment, external message |
| Executor | Perform explicitly granted execution or connector commit after approval token | Any standing permission; any action without grant and token |

The goal is not to make every agent weak. The goal is to make authority visible,
specific, revocable, and replayable.

## UI Observability Model

The UI should become a Workflow Run console, not only a generic session console.

Minimum objects to show:

- Workflow Pack installation and version.
- Onboarding readiness and connector quality.
- WorkflowDefinition and WorkflowRun status.
- WorkflowStepRun list and graph.
- Agents and current role status.
- Environment/runtime adapter binding.
- Session and SessionThread tree.
- Event stream with cursor and status transitions.
- Handoff chain.
- TaskGrant and scopes.
- Tool calls and policy decisions.
- Blocking approvals and approval tokens.
- Artifacts and final results.
- Worker lease/runtime logs.
- Memory writeback candidates.

The operator should answer these questions from one screen:

- Which workflow is running?
- Which agents are active?
- What is each agent doing now?
- Which grant allowed it?
- What memory and connector scope did it receive?
- What is blocked?
- What tool call or external effect is waiting for approval?
- What artifacts were produced?
- What can be replayed if something goes wrong?

## Example: Legal Digital Worker As A Workflow Pack

A legal "digital worker" should be a pack-driven workflow, not one employee-like
agent with broad authority.

```text
Legal Pack
  profiles:
    company legal policy
    jurisdiction rules
    approval matrix
    privilege scope
  workflows:
    contract intake
    redline review
    risk memo
    vendor DPA review
  agents:
    legal-orchestrator
    document-reader
    clause-extractor
    legal-analyzer
    memo-writer
    filing-executor
```

Flow:

1. User uploads contract or creates WorkItem.
2. Orchestrator creates WorkflowRun and grants reader only document access.
3. Reader extracts clauses into schema JSON.
4. Analyzer reviews structured clauses against profile and policy.
5. Writer creates draft memo or redline artifact.
6. Human counsel approves, edits, or rejects.
7. Executor only files/sends if an ApprovalCommitToken binds the exact action.

Legal memory is isolated by matter/client/privilege scope. Approved company
playbooks can be shared; raw matter facts and legal advice should not be shared
across unrelated workflows.

## Example: Social Media Or Ecommerce Automation

Social media and ecommerce should also be workflow packs, not broad employees.

Social media:

```text
Daily research trigger
  -> research-reader: collect allowed sources
  -> trend-analyzer: rank opportunities
  -> post-writer: draft posts
  -> approval console
  -> publish-executor: publish only with approval token
```

Ecommerce ads:

```text
Hourly metrics trigger
  -> metrics-reader: read ad/store metrics
  -> performance-analyzer: detect spend changes
  -> action-writer: draft budget recommendation
  -> approval console
  -> ad-executor: mutate spend only within grant and approval token
```

The runtime can support these. The first governed slice now exists: workflow
run objects, task-level grants, connector scopes, approval token binding, and
workflow UI are in the runtime path. The remaining work is deeper transition
policy, production connector adapters, provider-specific pack deployment, and
rich graph observability.

## Implementation Plan

### Slice 1: Architecture And Contract Alignment

- Add this architecture document.
- Keep `docs/workflow-pack-manifest-contract.md` as the pack validation
  contract.
- Keep `docs/architecture.md` as the Agent OS kernel overview.
- Add references from the overview docs to this workflow architecture.

Acceptance:

- Clear boundary between runtime, orchestrator, worker, agent, skill, grant,
  environment, and adapter.

### Slice 2: Workflow Run Store

- Status: implemented for definition/run/step persistence, APIs, root grant
  issuance, start-step materialization from `step_graph`, step update API,
  dependency-based graph advancement on step completion, and durable
  `WorkflowTransition` records for start, dependency, and completion events.
  Branch/skip/fail/retry/compensation policy scheduling remains pending.
- Add migrations and store modules for `workflow_definitions`,
  `workflow_runs`, `workflow_step_runs`, and `workflow_transitions`.
- Add read APIs for workflow run console.
- Link WorkflowRun to Session, WorkItem, PackInstallation, and AuditTrace.

Acceptance:

- Starting a workflow creates a durable run and primary session.
- UI/API can query run, steps, status, and linked session events.

### Slice 3: Pack Materialization

- Status: implemented as durable generic `workflow_pack_bindings` for manifest
  objects including workflows, agents, connectors, policies, evals, schemas,
  skills, profiles, onboarding schemas, and release gates. Dedicated provider
  deployment handles and schedule adapters remain pending.
- Stage pack into draft AgentVersion, WorkflowDefinition, ConnectorBinding,
  PolicyRevision, EvalSuite, and ScheduleBinding objects.
- Release activates those bindings.
- Rollback disables future entrypoints and schedules without deleting evidence.

Acceptance:

- `stage` is more than a status flip.
- Released pack workflows can be started by id.

### Slice 4: TaskGrant Foundation

- Status: implemented for root grants, child grant API, narrowing checks, and
  audit events; automatic handoff/step issuance now materializes specialist
  WorkflowStepRun and child TaskGrant records.
- Add TaskGrant table/model/store/API.
- Issue root grant on WorkflowRun creation.
- Issue narrowed child grant on handoff or step assignment.
- Persist `task_grant.issued`, `task_grant.checked`, and `task_grant.denied`
  events.

Acceptance:

- Child grants cannot expand parent grants.
- No grant means no tool/connector/memory access.

### Slice 5: MemoryScope, ToolScope, ConnectorScope Enforcement

- Status: implemented for workflow session loops and context packets requiring
  active TaskGrant authority. Tool scope, MCP connector allowlists, generic
  native connector id/operation/side-effect-class allowlists, MemoryScope trust
  thresholds, and memory writeback permission are enforced. MCP and native
  connector `commit_write` calls require ApprovalCommitToken exact binding.
  Production native connector transports still need service-specific
  reconciliation and rollback semantics.
- Apply grant checks before context packet creation.
- Apply grant checks before tool policy.
- Apply connector scope checks before MCP/native connector calls.
- Deny publish/pay/send/ad-spend mutations by default.

Acceptance:

- Worker cannot read memory outside scope.
- Writer cannot call connector commit by role default.
- Orchestrator cannot execute shell/Codex/browser unless explicitly granted.

### Slice 6: ApprovalCommitToken

- Status: implemented for `mcp.call`, `native.connector.call`, and
  external-write scoped commit tools when the active TaskGrant has
  `ConnectorScope.mode = commit_write`.
- Approval binds args hash, target binding, grant, tool call, and approver.
- Worker revalidates session/team MCP server+tool scope, runtime secret refs,
  active/non-expired TaskGrant liveness, ConnectorScope allowlists, and the
  token digest, then consumes the token immediately before the MCP Gateway call.
- Native connector bindings include connector id, operation, side-effect class,
  payload digest, amount/spend/account/resource fields where present, content
  digest where content-like payload is present, and context packet id where
  present.

Acceptance:

- Changing recipient, platform, account, or call args after approval fails
  closed.

### Slice 7: Runtime Adapter As Environment Path

- Promote Codex App Server / Codex CLI / Claude Code from low-level tool facade
  toward Environment-bound runtime adapter execution.
- Keep `codex.exec` and `agent_cli.exec` as compatibility paths.
- Normalize runtime turn events into WorkflowRun and SessionThread views.

Acceptance:

- A workflow step can run through an Environment adapter without the product
  semantics being "call codex.exec".

### Slice 8: Workflow Observability UI

- Add Workflow Run console.
- Show Pack, readiness, run status, step graph, agents, threads, grants,
  approvals, tool calls, artifacts, worker logs, and memory candidates.
- Keep infrastructure panels advanced.

Acceptance:

- User can see multiple agents running, what they are doing, what is blocked,
  and what result each step produced.

### Slice 9: AI Governance Pack E2E

- Use `packs/ai-governance` as the first full pack.
- Prove install -> onboarding -> connector quality -> stage -> release ->
  start workflow -> handoff -> draft artifact -> approval -> replay.

Acceptance:

- One complete governed managed workflow runs without relying on fake static UI
  state.

## Required Tests

- Pack stage materializes draft runtime objects.
- WorkflowRun creates primary session and root grant.
- Child grant cannot expand parent grant.
- ContextPacket only includes MemoryScope-allowed objects.
- Reader cannot write.
- Analyzer only receives schema-validated reader output when configured that
  way.
- Writer cannot call connector commit.
- Orchestrator cannot call shell/Codex/agent CLI without executor grant.
- Handoff target, intent, schema, and risk are validated.
- High-risk handoff enters Approval lifecycle.
- MCP/native connector call checks ConnectorScope.
- Publish/pay/send/ad-spend mutation is denied by default.
- ApprovalCommitToken binds exact side effect digest.
- Worker revalidates grant before execution.
- ToolCall -> Approval -> Grant -> ContextPacket -> Artifact is traceable.
- MemoryWritebackCandidate cannot become high-trust memory without review.

## Non-Goals

- Do not build a single "digital employee" abstraction as the core product.
- Do not let Workflow Packs become the Agent OS itself.
- Do not put business action authority in prompts.
- Do not rely on UI hiding to enforce safety.
- Do not make connector writes safe merely because an agent asked nicely.
- Do not promote raw session summaries into shared memory automatically.
- Do not constrain MandoForge to the single-level callable-agent limit of a
  specific external managed-agent product.

## MVP Choice

The first E2E pack should be AI Governance Pack.

Reason:

- It matches MandoForge's current strengths: draft outputs, approval, audit,
  connector provenance, policy gating, evals, and release gates.
- It avoids the operational fragility of social publishing or ad mutation as
  the first proof.
- It avoids coupling the runtime architecture to legal-domain complexity too
  early.

Legal, social media, and ecommerce packs should come after the workflow runtime
proves one complete governed run.
