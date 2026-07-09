# Full Agent OS Narrative Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the public MandoForge architecture docs, roadmap, Remote Computer plan, and verification script with the approved Full Agent OS narrative.

**Architecture:** This is the first implementation slice for `docs/superpowers/specs/2026-07-09-full-agent-os-narrative-design.md`. It deliberately updates documentation and a lightweight verification gate only; runtime code, schemas, and UI are separate slices so K Agent, TaskGrant, Ontology Action Contract, and WorkflowPack changes can each receive their own focused plan.

**Tech Stack:** Markdown, Bash, existing MandoForge docs, existing evidence-gate script conventions.

## Global Constraints

- MandoForge is a runtime-centered Enterprise Agent OS, not a Claude Managed Agents clone, a Palantir AIP clone, or a general enterprise data platform.
- Product center: Manager Runtime and Managed Runtime.
- CMA reference boundary: `Agent -> Environment -> Session -> Events -> Threads`.
- AIP reference boundary: Context Engineering, ontology-backed actions, purpose-based governance, package/release/deploy, Human+AI applications, and operational automation.
- K Agent belongs under Environment Scheduling and Execution Substrate; it does not own ManagerPlan, Policy, Approval, TaskGrant, Ontology validity, WorkflowPack release, or audit truth.
- Ontology defines action validity, not execution authority.
- No Rust runtime code, database migrations, API route changes, UI changes, or Kubernetes manifest changes in this plan.

---

## Scope Check

The approved spec covers multiple independent subsystems. This plan implements only the narrative-alignment slice:

```text
README.md
docs/architecture.md
docs/stage2-stage3-roadmap.md
docs/agent-remote-computer-plan.md
scripts/verify-agent-os-narrative.sh
```

Separate plans should cover runtime adapter consolidation, K Agent execution claims, Manager Runtime materialization, Ontology Action Contract enforcement, and Pack/Release evidence.

## File Structure

### Modify

- `README.md`
  - Align the top-level Agent OS stack with the eight-layer Full Agent OS narrative.
  - Add a short Manager Runtime center statement.
  - Link to the approved spec and verification gate.

- `docs/architecture.md`
  - Replace the product stack with the eight-layer architecture.
  - Add ownership rules for Manager Runtime, Governance, Ontology Action Contract, Environment Scheduling, K Agent, and Execution Substrate.

- `docs/stage2-stage3-roadmap.md`
  - Reframe workstreams around the implementation phases from the approved spec.
  - Preserve existing baseline facts while adding the Environment Scheduling + K Agent phase.

- `docs/agent-remote-computer-plan.md`
  - Make Remote Computer subordinate to Environment Scheduling and K Agent.
  - Replace the older Remote Computer Manager-centered chain with the new Manager Runtime to K Agent boundary.

### Create

- `scripts/verify-agent-os-narrative.sh`
  - Check that the narrative boundary terms are present in the public docs.
  - Fail closed if the docs drift back to Remote Computer-centered, ontology-centered, or backend-centered wording.

### Test

- Run `bash scripts/verify-agent-os-narrative.sh`.
- Run `git diff --check`.

---

### Task 1: Align README With Full Agent OS Narrative

**Files:**

- Modify: `README.md`

**Interfaces:**

- Consumes: approved spec `docs/superpowers/specs/2026-07-09-full-agent-os-narrative-design.md`
- Produces: README sections that following tasks and verification script can assert against

- [ ] **Step 1: Replace the Agent OS Stack block**

In `README.md`, replace the existing `## Agent OS Stack` stack diagram with this block:

````markdown
## Agent OS Stack

MandoForge is a runtime-centered Enterprise Agent OS. The center is the Manager
Runtime and Managed Runtime: who started work, who decomposed it, who was
assigned, which agent ran, which environment ran it, which context it saw, which
tool it called, whether the call was authorized, who approved it, what happened,
and whether the run can be replayed, released, or rolled back.

```text
Work Surfaces
  Feishu / Slack / GitHub / Jira / Linear / Email / Browser
        |
Collaboration Layer
  WorkItem / Project / Assignment / Review / Activity Feed / Squad
        |
Manager Agent Layer
  Intake / Plan / Decompose / Route / Delegate / Escalate / Review
        |
Managed Runtime Layer
  Agent / Environment / Session / Events / Threads / Runtime Turns
        |
Governance Layer
  Policy / Approval / RBAC / TaskGrant / Audit / Eval / Release / Rollback
        |
Ontology Action Contract Layer
  Business Object / Rule / Relation / Metric / Action / Tool Binding / Validation
        |
Environment Scheduling Layer
  Environment Work Queue / K Agent / Worker Lease / Sandbox Lifecycle / CLI Dispatch
        |
Execution Substrate
  Codex / Claude Code / CMA / MCP / SQL / Shell / Remote Computer / APIs
```

Claude Managed Agents is a runtime reference for `Agent -> Environment ->
Session -> Events -> Threads`. Palantir AIP is an enterprise operation reference
for context engineering, governed actions, package/release/deploy, and
Human+AI application surfaces. Neither is the product center.
````

Expected result: README makes Manager Runtime and Managed Runtime the center and includes the eight-layer stack.

- [ ] **Step 2: Add design reference link near the docs link list**

In `README.md`, near the existing architecture links, add:

```markdown
- [Full Agent OS Narrative Design](docs/superpowers/specs/2026-07-09-full-agent-os-narrative-design.md)
```

Expected result: readers can find the approved narrative spec from README.

- [ ] **Step 3: Verify README contains required narrative anchors**

Run:

```bash
rg -n "runtime-centered Enterprise Agent OS|Manager Runtime and Managed Runtime|Environment Scheduling Layer|Ontology Action Contract Layer|K Agent" README.md
```

Expected: five matching lines.

- [ ] **Step 4: Commit Task 1**

Run:

```bash
git add README.md
git commit -m "Align README with full Agent OS narrative"
```

Expected: one commit containing only `README.md`.

---

### Task 2: Align Architecture And Roadmap Docs

**Files:**

- Modify: `docs/architecture.md`
- Modify: `docs/stage2-stage3-roadmap.md`

**Interfaces:**

- Consumes: README narrative anchors from Task 1
- Produces: architecture and roadmap docs with the same layer names and implementation phases

- [ ] **Step 1: Replace the architecture product stack**

In `docs/architecture.md`, replace the existing `## Product Stack` diagram with this block:

````markdown
## Product Stack

MandoForge is organized around Manager Runtime and Managed Runtime, not around a
single backend, data platform, or sandbox substrate.

```text
Work Surfaces
  Feishu / Slack / GitHub / Jira / Linear / Email / Browser
        |
Collaboration Layer
  WorkItem / Project / Assignment / Review / Activity Feed / Squad
        |
Manager Agent Layer
  Intake / Plan / Decompose / Route / Delegate / Escalate / Review
        |
Managed Runtime Layer
  Agent / Environment / Session / Events / Threads / Runtime Turns
        |
Governance Layer
  Policy / Approval / RBAC / TaskGrant / Audit / Eval / Release / Rollback
        |
Ontology Action Contract Layer
  Business Object / Rule / Relation / Metric / Action / Tool Binding / Validation
        |
Environment Scheduling Layer
  Environment Work Queue / K Agent / Worker Lease / Sandbox Lifecycle / CLI Dispatch
        |
Execution Substrate
  Codex / Claude Code / CMA / MCP / SQL / Shell / Remote Computer / APIs
```

The optional desktop shell is a local product surface for operators. It owns tray
integration, native notification forwarding, autostart controls, and loopback
WebView startup, but it does not own the Agent OS runtime or policy boundary.
````

Expected result: `docs/architecture.md` has the same eight-layer stack as README.

- [ ] **Step 2: Add ownership rules to architecture**

In `docs/architecture.md`, replace the `Core ownership:` list with:

```markdown
Core ownership:

- Manager Agents coordinate WorkItems, Assignments, Reviews, ManagerPlans, and
  child specialist threads. They do not own a second runtime orchestrator.
- MandoForge Managed Runtime owns sessions, event logs, threads, runtime turns,
  tool calls, approvals, artifacts, cursor/resume, streaming, and worker leases.
- Governance owns Policy, RBAC, TaskGrant, Approval, ApprovalCommitToken, Audit,
  Eval, Release, and Rollback.
- Ontology Action Contract defines business objects, rules, action validity,
  permission contracts, tool bindings, and validation rules. It does not grant
  execution authority by itself.
- Environment Scheduling owns environment work queues, K Agent claims, worker
  leases, sandbox lifecycle, CLI dispatch, heartbeat, cleanup, and artifact
  sync. K Agent does not own ManagerPlan, Policy, Approval, TaskGrant, Ontology
  validity, WorkflowPack release, or audit truth.
- Codex CLI, Claude Code CLI, Claude Managed Agents, Codex App Server, Remote
  Computer, MCP, SQL, shell, and native APIs are execution substrates or runtime
  adapters called by MandoForge.
```

Expected result: architecture doc states what each layer owns and does not own.

- [ ] **Step 3: Replace roadmap architecture diagram**

In `docs/stage2-stage3-roadmap.md`, replace the current `## Architecture` diagram and the two paragraphs immediately below it with this block:

````markdown
## Architecture

The product architecture follows the Full Agent OS narrative:

```text
Work Surfaces
  Feishu / Slack / GitHub / Jira / Linear / Email / Browser
        |
Collaboration Layer
  WorkItem / Project / Assignment / Review / Activity Feed / Squad
        |
Manager Agent Layer
  Intake / Plan / Decompose / Route / Delegate / Escalate / Review
        |
Managed Runtime Layer
  Agent / Environment / Session / Events / Threads / Runtime Turns
        |
Governance Layer
  Policy / Approval / RBAC / TaskGrant / Audit / Eval / Release / Rollback
        |
Ontology Action Contract Layer
  Business Object / Rule / Relation / Metric / Action / Tool Binding / Validation
        |
Environment Scheduling Layer
  Environment Work Queue / K Agent / Worker Lease / Sandbox Lifecycle / CLI Dispatch
        |
Execution Substrate
  Codex / Claude Code / CMA / MCP / SQL / Shell / Remote Computer / APIs
```

Claude Managed Agents remains a useful reference for the Managed Runtime Layer:
Agent, Environment, Session, Events, and Threads. It is not the complete
MandoForge architecture.

Palantir AIP remains a useful reference for enterprise operation patterns:
context engineering, purpose-based controls, package/release/deploy, and
Human+AI operation surfaces. MandoForge does not copy AIP's ontology-centered
data-platform boundary.
````

Expected result: roadmap architecture matches the approved narrative.

- [ ] **Step 4: Add implementation phase summary to roadmap**

In `docs/stage2-stage3-roadmap.md`, after `## Near-Term Priority`, add:

```markdown
## Full Agent OS Implementation Phases

1. Runtime Contract: keep event-driven sessions, runtime turns, `requires_action`,
   and environment-bound runtime adapters on the single session-loop path.
2. Environment Scheduling + K Agent: isolate sandbox and CLI dispatch from
   business authority while preserving worker lease, event, artifact, and audit
   evidence.
3. Manager Runtime: make Manager Agents operate on WorkItems, ManagerPlans,
   Assignments, Reviews, WorkflowRuns, SessionThreads, and TaskGrants.
4. Ontology Action Contract: use Ontology for action validity, rules, tool
   bindings, and validation; keep TaskGrant, Policy, Approval, and Tool Router
   as execution authority.
5. Pack / Release / Evidence: make WorkflowPack, DomainPack, AgentVersion,
   EnvironmentProfile, OntologyActionContract, ToolSpec, EvalGate, Release, and
   Rollback installable and auditable.
```

Expected result: roadmap has the same five phases as the approved spec.

- [ ] **Step 5: Verify architecture and roadmap anchors**

Run:

```bash
rg -n "Environment Scheduling Layer|K Agent|Ontology Action Contract|Full Agent OS Implementation Phases|Palantir AIP remains" docs/architecture.md docs/stage2-stage3-roadmap.md
```

Expected: matching lines in both docs.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add docs/architecture.md docs/stage2-stage3-roadmap.md
git commit -m "Align architecture docs with full Agent OS layers"
```

Expected: one commit containing only `docs/architecture.md` and `docs/stage2-stage3-roadmap.md`.

---

### Task 3: Reframe Remote Computer Under K Agent Boundary

**Files:**

- Modify: `docs/agent-remote-computer-plan.md`

**Interfaces:**

- Consumes: layer definitions from Task 2
- Produces: Remote Computer plan that places K Agent and Remote Computer under Environment Scheduling

- [ ] **Step 1: Replace Managed Agents Alignment section**

In `docs/agent-remote-computer-plan.md`, replace the `## Managed Agents Alignment` section with:

````markdown
## Full Agent OS Alignment

This plan is subordinate to the Full Agent OS narrative in
[Full Agent OS Narrative Design](superpowers/specs/2026-07-09-full-agent-os-narrative-design.md)
and the managed-agent resource model in
[Claude Managed Agents Alignment](claude-managed-agents-alignment.md).

Remote Computer remains the target isolated execution substrate, but it is not
the top-level product object. The top-level runtime chain remains:

```text
Agent -> Environment -> Session -> Events -> Threads
```

Remote Computer should be reached through:

```text
Managed Runtime
  -> Environment Scheduling Layer
  -> K Agent
  -> Remote Computer lease / Pod / warm-pool slot
  -> CLI adapter or approved tool execution
  -> runtime events, artifacts, usage, and audit
```

K Agent is the execution-side controller for sandbox and CLI dispatch. It may
claim work, select Pods, prepare workspace/state mounts, launch Codex or Claude
Code, stream runtime output, sync artifacts, and clean up leases. It does not
own ManagerPlan, WorkItem routing authority, TaskGrant creation or expansion,
Policy decisions, Approval decisions, Ontology validity, WorkflowPack release,
or audit truth.
````

Expected result: Remote Computer is clearly under Environment Scheduling and K Agent.

- [ ] **Step 2: Replace core runtime shape**

In `docs/agent-remote-computer-plan.md`, replace the `The core runtime shape becomes:` diagram with:

````markdown
The core execution shape becomes:

```text
WorkItem / ManagerPlan / WorkflowRun / Session
  -> TaskGrant / Policy / Approval
  -> Session Event / SessionLoopJob / ExecutionJob
  -> Environment Scheduling
  -> K Agent worker claim
  -> Remote Computer lease or Pod claim
  -> Mounted workspace and state filesystem
  -> Sandbox / Codex / Claude Code / shell / tool execution
  -> Runtime events + Artifact + Audit sync
```
````

Expected result: the plan no longer centers Remote Computer Manager before governance.

- [ ] **Step 3: Add K Agent ownership guardrail**

In `docs/agent-remote-computer-plan.md`, before `## Current State`, add:

```markdown
## K Agent Guardrail

K Agent must stay an Environment Scheduling component:

- It may claim approved work for an Environment.
- It may select or create an execution substrate.
- It may dispatch CLI adapters and approved tool runners.
- It may report heartbeats, runtime events, artifacts, usage, and cleanup state.
- It must not grant authority, expand TaskGrants, approve high-risk actions,
  decide Ontology validity, release WorkflowPacks, or become the audit source of
  truth.

If an implementation needs K Agent to make a business decision, that
decision belongs in Manager Runtime, Governance, or Ontology Action Contract
first, and K Agent should receive only the approved execution envelope.
```

Expected result: Remote Computer docs define the K Agent boundary explicitly.

- [ ] **Step 4: Verify Remote Computer/K Agent anchors**

Run:

```bash
rg -n "Full Agent OS Alignment|Environment Scheduling Layer|K Agent Guardrail|approved execution envelope|Remote Computer lease or Pod claim" docs/agent-remote-computer-plan.md
```

Expected: five matching lines.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add docs/agent-remote-computer-plan.md
git commit -m "Reframe Remote Computer under K Agent scheduling"
```

Expected: one commit containing only `docs/agent-remote-computer-plan.md`.

---

### Task 4: Add Narrative Verification Gate

**Files:**

- Create: `scripts/verify-agent-os-narrative.sh`

**Interfaces:**

- Consumes: narrative anchors introduced by Tasks 1-3
- Produces: a repeatable script that fails if public docs drift away from the approved boundaries

- [ ] **Step 1: Create verification script**

Create `scripts/verify-agent-os-narrative.sh` with this exact content:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

require_pattern() {
  local file="$1"
  local pattern="$2"
  if ! grep -Fq "$pattern" "$file"; then
    echo "missing required narrative anchor in $file: $pattern" >&2
    exit 1
  fi
}

require_absent() {
  local file="$1"
  local pattern="$2"
  if grep -Fq "$pattern" "$file"; then
    echo "forbidden narrative drift in $file: $pattern" >&2
    exit 1
  fi
}

require_pattern README.md "runtime-centered Enterprise Agent OS"
require_pattern README.md "Manager Runtime and Managed Runtime"
require_pattern README.md "Environment Scheduling Layer"
require_pattern README.md "Ontology Action Contract Layer"
require_pattern README.md "K Agent"

require_pattern docs/architecture.md "Environment Scheduling Layer"
require_pattern docs/architecture.md "K Agent does not own ManagerPlan"
require_pattern docs/architecture.md "Ontology Action Contract defines business objects"

require_pattern docs/stage2-stage3-roadmap.md "Full Agent OS Implementation Phases"
require_pattern docs/stage2-stage3-roadmap.md "Environment Scheduling + K Agent"
require_pattern docs/stage2-stage3-roadmap.md "Ontology Action Contract"

require_pattern docs/agent-remote-computer-plan.md "Full Agent OS Alignment"
require_pattern docs/agent-remote-computer-plan.md "K Agent Guardrail"
require_pattern docs/agent-remote-computer-plan.md "approved execution envelope"

require_absent README.md "Ontology is the product center"
require_absent docs/architecture.md "Remote Computer is the top-level product object"
require_absent docs/agent-remote-computer-plan.md "K Agent owns Policy"

echo "agent_os_narrative=ok"
```

Expected result: script checks narrative anchors without needing the API server.

- [ ] **Step 2: Make the script executable**

Run:

```bash
chmod +x scripts/verify-agent-os-narrative.sh
```

Expected: script has executable bit.

- [ ] **Step 3: Run the verification script**

Run:

```bash
scripts/verify-agent-os-narrative.sh
```

Expected:

```text
agent_os_narrative=ok
```

- [ ] **Step 4: Run diff check**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add scripts/verify-agent-os-narrative.sh
git commit -m "Add Agent OS narrative verification gate"
```

Expected: one commit containing only `scripts/verify-agent-os-narrative.sh`.

---

### Task 5: Final Narrative Review And Handoff

**Files:**

- Inspect: `README.md`
- Inspect: `docs/architecture.md`
- Inspect: `docs/stage2-stage3-roadmap.md`
- Inspect: `docs/agent-remote-computer-plan.md`
- Inspect: `scripts/verify-agent-os-narrative.sh`

**Interfaces:**

- Consumes: all previous task commits
- Produces: reviewed narrative alignment and a list of next plans to write

- [ ] **Step 1: Run the full narrative verification**

Run:

```bash
scripts/verify-agent-os-narrative.sh
git diff --check
```

Expected:

```text
agent_os_narrative=ok
```

and no `git diff --check` output.

- [ ] **Step 2: Review the final branch diff**

Run:

```bash
git show --stat --oneline HEAD~3..HEAD
git diff HEAD~3..HEAD -- README.md docs/architecture.md docs/stage2-stage3-roadmap.md docs/agent-remote-computer-plan.md scripts/verify-agent-os-narrative.sh
```

Expected: only the four docs and one script changed across the three implementation commits.

- [ ] **Step 3: Record next implementation plans**

Add this section to the end of `docs/stage2-stage3-roadmap.md`:

```markdown
## Follow-Up Implementation Plans

The Full Agent OS narrative should be implemented through separate focused
plans:

1. Runtime adapter consolidation: make Environment runtime binding the only
   product entrypoint for managed runtime execution while keeping `agent_cli.exec`
   and `codex.exec` as compatibility facades.
2. Environment Scheduling + K Agent: define the Environment Work Queue and
   K Agent claim, lease, heartbeat, sandbox dispatch, and runtime event return
   contract.
3. Manager Runtime materialization: connect WorkItem, ManagerPlan, Assignment,
   Review, WorkflowRun, SessionThread, and TaskGrant with evidence gates.
4. Ontology Action Contract enforcement: make action validity a checked input to
   Tool Router without letting ontology bypass TaskGrant, Policy, or Approval.
5. Pack / Release / Evidence: make WorkflowPack, DomainPack, AgentVersion,
   EnvironmentProfile, OntologyActionContract, ToolSpec, EvalGate, Release, and
   Rollback auditable as product capabilities.
```

Expected result: roadmap names the next implementation plans without putting their code changes into this documentation slice.

- [ ] **Step 4: Re-run verification**

Run:

```bash
scripts/verify-agent-os-narrative.sh
git diff --check
```

Expected:

```text
agent_os_narrative=ok
```

and no `git diff --check` output.

- [ ] **Step 5: Commit final roadmap handoff**

Run:

```bash
git add docs/stage2-stage3-roadmap.md
git commit -m "Document Agent OS follow-up implementation plans"
```

Expected: one commit containing only `docs/stage2-stage3-roadmap.md`.

---

## Final Verification

After all tasks:

```bash
scripts/verify-agent-os-narrative.sh
git status --short
git log --oneline -5
```

Expected:

```text
agent_os_narrative=ok
```

`git status --short` should be empty. The five most recent commits should include:

```text
Document Agent OS follow-up implementation plans
Add Agent OS narrative verification gate
Reframe Remote Computer under K Agent scheduling
Align architecture docs with full Agent OS layers
Align README with full Agent OS narrative
```

## Next Plan Boundaries

Do not continue from this plan directly into runtime code. Write separate specs
or plans for:

- Runtime adapter consolidation.
- Environment Scheduling + K Agent.
- Manager Runtime materialization.
- Ontology Action Contract enforcement.
- Pack / Release / Evidence.
