# MandoForge Roadmap v2

MandoForge Roadmap v2 is the runtime-first implementation of the original Enterprise Agent OS plan.

The original product direction is still the source of truth:

```text
Enterprise Agent OS =
Data / Semantic Foundation
+ Managed Agent Runtime
+ Manager Agent
+ Specialist Agents
+ Tool / Action Runtime
+ Governance
```

The current repo implements that plan from the runtime upward. Stage 1-3 prove the governed runtime, queue, Remote Computer, and Pack substrate. Stage 4 returns to the original Agent OS control-plane design by making Manager Agent, Managed Agents, runtime profiles, handoffs, and a minimal semantic kernel first-class. Stage 5 expands the minimal semantic kernel into the full Context OS.

## Layer Model

MandoForge should be understood as an Agent OS substrate, not as a single vertical agent.

```text
Workflow Packs / Domain Packs
  - Legal Pack
  - Coding Pack
  - Sales Pack
  - Finance Pack
  - Ops Pack

Semantic Layer / Context OS
  - semantic objects
  - context packets
  - memory writeback
  - provenance and freshness

Managed Agent Control Plane
  - Manager Agent
  - Specialist Agents
  - runtime profiles
  - handoffs and assignments

Remote Computer / Action Runtime
  - isolated execution
  - leases
  - sidecars
  - artifact sync

Governed Runtime
  - worker queue
  - RBAC
  - provider governance
  - eval and release gates

Agent OS Kernel
  - session event log
  - tool router
  - policy
  - approval
  - audit
  - artifacts
  - timeline replay
```

Workflow Packs are applications that run on the Agent OS. They are not the Agent OS itself. A legal pack in the style of `claude-for-legal` should install named agents, practice profiles, skills, connector requirements, scheduled workflows, review gates, and domain-specific output rules on top of MandoForge.

## Stage 1: Agent OS Kernel

Status: complete.

Stage 1 proves the minimum governed agent loop:

- Agent and Agent Version records
- Session lifecycle
- append-only session event log
- tool calls
- Tool Router
- Policy Engine
- Approval Engine
- audit logs
- artifacts
- timeline replay
- basic read/write/query/shell/Codex tools

Outcome:

```text
An agent task can be started, routed through tools, paused for approval, resumed, audited, and replayed.
```

## Stage 2: Governed Runtime

Status: repo-controlled runtime complete; production promotion evidence remains environment-specific.

Stage 2 turns the kernel into an enterprise governed runtime:

- Postgres-backed storage
- execution queue and worker handoff
- RBAC with org/team/project scope
- provider governance
- secret references and Vault boundary
- MCP gateway boundary
- policy rollout
- approval notifications
- eval and release gates
- observability and evidence gates
- production adoption runbooks

Outcome:

```text
The Agent OS is no longer a demo loop. It has durable runtime state, governance, release gates, and evidence surfaces.
```

## Stage 3: Remote Computer + Pack Substrate

Status: Whiskey single-node pilot complete; multi-node production Remote Computer state is not complete.

Stage 3 implements the action runtime and installable pack substrate:

- Remote Computer records
- Remote Computer leases
- session attachments
- execution job assignments
- warm-pool Pods
- assigned Pod execution path
- artifact sidecar and discovery
- state locks
- sidecar heartbeat and recovery planning
- Workflow Pack manifest validation
- Workflow Pack install, stage, release, rollback, archive
- onboarding profiles
- connector quality gates

Outcome:

```text
Approved actions can run in an isolated Remote Computer path without bypassing policy, approval, audit, or timeline. Workflow Packs can be installed and governed as packages, but domain packs are still applications above the OS.
```

Boundary:

```text
Whiskey k3s local-hostpath validates the single-node pilot chain.
It does not prove multi-node distributed state with JuiceFS, CephFS, Longhorn, or equivalent shared storage.
```

## Stage 4: Managed Agent Control Plane + Manager Agent + Minimal Semantic Kernel

Status: next active build stage.

Stage 4 is the main correction back to the original Agent OS plan. It must not stop at "configure a backend-coder." The goal is to make MandoForge manage Manager Agents and Specialist Agents as first-class resources.

### Stage 4.1 Agent Runtime Profile

Move runtime backend configuration out of environment-only settings and into managed runtime profiles.

Current repo slice:

- `agent_runtime_profiles` is now a tenant-scoped Agent OS resource.
- `GET/POST /api/agent-runtime-profiles` and `GET /api/agent-runtime-profiles/{id}` expose the first control-plane API.
- `agent_cli.exec` resolves an enabled managed `agent_cli` profile before falling back to legacy environment allowlist configuration.
- profiles with `remote_computer_required: true` fail closed on local execution and must use the Remote Computer path.

Examples:

```yaml
runtime_profile:
  name: codex-worker
  type: agent_cli
  command_ref: codex
  default_args:
    - exec
  remote_computer_required: true
```

Profiles should support Codex, Claude Code, Gemini, OpenCode, Aider, App Server strategies, and future hosted runtimes through a governed allowlist.

### Stage 4.2 Managed Agent Registry

Managed Agents should bind together:

- name and role
- Manager or Specialist kind
- runtime profile
- tool allowlist
- MCP servers
- skills
- Workflow Pack memberships
- approval policy
- Remote Computer profile
- semantic scopes
- version and release state

### Stage 4.3 Minimal Semantic Kernel

Stage 4 needs a minimal semantic kernel before the full Stage 5 semantic layer exists.

Managed Agent config should include explicit scopes:

```yaml
semantic_scopes:
  project_scope:
  repo_scope:
  service_scope:
  workflow_scope:
  policy_scope:
  memory_scope:
```

This prevents Agent Registry from becoming only a runtime/tool table. The Manager Agent must use scopes when selecting specialists and assembling the first context packet.

### Stage 4.4 Manager Agent Planner

The Manager Agent is the task-control brain:

- intake user or system task
- classify task intent and risk
- decompose task into steps
- select Specialist Agent by scope and capability
- decide whether Remote Computer is required
- create handoff / assignment records
- request approval for high-risk actions
- review results
- escalate to humans when blocked

### Stage 4.5 Agent Handoff / Assignment

Handoff must become the product surface for Manager Agent -> Specialist Agent delegation:

- typed intent
- source and target agents
- payload schema
- risk level
- approval requirement
- semantic scopes
- runtime profile
- Remote Computer assignment
- result and review status
- audit trace

### Stage 4.6 First Demo: backend-coder

`backend-coder` is the first demonstration Managed Agent, not the product endpoint.

It should prove:

- Managed Agent config
- runtime profile
- minimal semantic scopes
- Manager Agent assignment
- Remote Computer execution
- artifact output
- audit and timeline replay

Outcome:

```text
MandoForge can receive a task, let a Manager Agent choose a Specialist Agent, bind the task to semantic scope and runtime profile, execute through the governed runtime, and review the result.
```

## Stage 5: Full Semantic Layer / Context OS

Status: not yet implemented beyond early source, artifact, session, Workflow Pack, and Remote Computer state primitives.

Stage 5 expands the minimal semantic kernel into a full Context OS:

- semantic sources
- semantic objects
- semantic links
- semantic snapshots
- ingestion jobs
- context packet builder
- memory writeback
- trust and freshness policy
- provenance
- ontology
- retrieval and ranking

The target object model:

```yaml
semantic_object:
  type: decision | runbook | code_module | workflow | policy | memory | artifact
  source: github | repo_docs | feishu | mcp | session | upload
  uri: ...
  trust_level: high | medium | low
  freshness: current | stale | unknown
  related_to:
    - agent_id
    - project_id
    - repo_path
    - workflow_id
  provenance:
    observed_at:
    verified_by:
    evidence_ref:
```

The target runtime flow:

```text
task
-> semantic context packet
-> Manager Agent planning
-> Specialist Agent assignment
-> Remote Computer execution
-> artifact / audit / review
-> memory writeback
```

Outcome:

```text
Agents no longer start from random file reads or chat-only context. MandoForge generates governed, source-attributed, freshness-aware context packets and writes durable organizational memory after execution.
```

## Workflow Packs / Domain Packs

Domain packs sit above Stage 3-5. They should never be treated as the OS itself.

A pack can include:

- named Manager and Specialist Agents
- practice or domain profiles
- commands and workflows
- skills
- connector requirements
- MCP server bindings
- scheduled tasks
- review gates
- eval fixtures
- release gates
- semantic source requirements
- output style rules

Examples:

- Legal Pack
- Coding Pack
- Sales Pack
- Finance Pack
- Ops Pack
- AI Governance Pack

The `claude-for-legal` style is a useful reference for pack shape: practice profiles, named agents, connectors, scheduled managed-agent workflows, and review gates. MandoForge should provide the OS substrate those packs install into.

## Current Priority

The next repo work should prioritize Stage 4:

1. Agent Runtime Profile
2. Managed Agent Registry
3. Minimal Semantic Kernel
4. Manager Agent Planner
5. Agent Handoff / Assignment
6. backend-coder demo as validation only

Stage 5 should begin with schema and context packet skeletons, but it should not block Stage 4.
