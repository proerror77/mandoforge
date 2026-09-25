# Ontology-first business runtime

A business workflow can require its Agent to obtain the assigned WorkItem,
customer facts, rules, relationships and registered Action contracts from
Ontology before proposing an effect. MandoForge owns authority, approval,
execution and result acceptance. The first adapter is local native Codex CLI;
this path does not depend on AX.

## Trusted entrypoint

An administrator registers an Ontology SDK application for an Agent using
`agent_id`. Its subject is derived by the server as `agent-runtime:<id>`.
A released native workflow binds that application's ID, explicit source object
IDs and completion requirements in `handoff_rules.root_task_grant`:

```json
{
  "semantic_scopes": {
    "domain_scope": "customer_success",
    "workflow_scope": "customer-followup",
    "share_policy": "tenant_only"
  },
  "tool_scope": {
    "read": ["ontology.context.read", "ontology.action.result.read"],
    "write": ["codex.exec", "ontology.action.execute"]
  },
  "approval_policy": {
    "ontology_runtime": {
      "application_id": "<registered application UUID>",
      "required_source_object_ids": ["<customer UUID>"]
    },
    "ontology_consumer_scope": {
      "objects": ["Customer", "FollowupPolicy"],
      "relations": ["<published relation API name>"],
      "actions": ["<published draft Action API name>", "<published closeout Action API name>"],
      "object_ids": ["<customer UUID>", "<policy UUID>"]
    }
  }
}
```

Start it through `POST /api/workflow-runs` with `source_work_item_id`.
The server derives the WorkItem binding and pins the published Ontology release.
Caller-provided execution text cannot replace these bindings. Child grants cannot
drop or widen inherited business authority. Unsupported delegated/remote business
adapters fail explicitly; ordinary engineering sessions retain their existing
interfaces without receiving this business capability.

## Agent tools and evidence

The native worker runs Codex under its existing session-loop lease. Each model
turn has a private loopback HTTP MCP endpoint and expiring bearer capability.
The server fixes the tenant, Agent, session, grant, context and worker claim;
tool arguments cannot override them. This bearer namespace is rejected by the
general API even when local development authentication is enabled.

The Agent receives three tools:

1. `ontology_read_context`: returns the objective and scoped data/contracts;
   persists a read receipt linked to the actual tool invocation.
2. `ontology_propose_action`: uses the receipt to submit a registered Action.
3. `ontology_read_action_result`: reads committed business state and records a
   separate result-read receipt. Pending or failed Actions cannot produce a
   successful result-read receipt.

Read evidence includes tenant/session/grant/application identity, context and
release versions, scoped object and observed relationship versions, WorkItem
version and expiry. Checks run before proposal and again inside the existing
approved-effect transaction. Source data or grant changes invalidate old evidence.
The check proves data was delivered through an Agent-origin tool call; it does
not prove that the model's interpretation of every business rule is correct.

Generic shell, browser, app, plugin and delegation tools are disabled for this
Codex process. The child receives a small environment allowlist and no platform
DB/admin/worker credentials. Existing native Codex login is reused in place;
credentials are not copied into a workspace or container. This is a supported
CLI capability boundary, not a separate OS/container isolation product.

## Internal business Actions

Two explicitly registered, approval-required `local_serializable` executors are
supported:

- `internal_followup_draft`, target `FollowupDraft`, with declared source object
  type. Creates an internal draft for the bound WorkItem and authorized source.
  Identity is deterministic for WorkItem/source. Repeating the same approved
  effect reuses the object; conflicting content or authority is rejected.
- `internal_work_item_closeout`, target `WorkItem`. Requires successful approved
  draft Actions and Agent result-read receipts for every required source. It
  closes the WorkItem and updates its semantic projection and activity together.

An Action awaiting approval ends the Agent turn and releases the worker lease.
Normal approval/execution events resume the same workflow. Final completion
requires the closeout Action and a subsequent Agent readback of its committed
state. Model text, a process exit or an approved Review alone cannot complete it.
Business effects, execution receipts, tool results and audit records share the
existing transaction. External executors remain proposal-only.

## Verification and limits

The focused `ontology_business` tests include the in-memory control contract,
private MCP identity/claim boundaries and opt-in Postgres tests. The ignored
`ontology_business_real_codex_native_worker_closes_work_item` test requires
`MANDOFORGE_TEST_POSTGRES_URL`, `MANDOFORGE_RUN_REAL_CODEX=1`, an installed Codex
supporting the configured feature flags, and an existing local login. Optional
`MANDOFORGE_BUSINESS_EVIDENCE_DIR` exports the synthetic run evidence.

The real test registers initial synthetic data, starts a normal workflow and
uses an independent test Approver through the normal approval API. It does not
query Ontology, propose Actions or close the WorkItem on the Agent's behalf.
Test approvals use explicit local development authentication and must not be
represented as production human-approval evidence.

The current adapter fails closed when a monetary budget is configured because
priced native Codex metering is not implemented here. Turn, tool and runtime
limits continue to apply. This release does not establish distributed scale,
all CLI/provider compatibility, production identity/deployment readiness or real
external business writeback. Production claims still require the enterprise
completion contract and target-specific runtime/readback evidence.

The Codex adapter allows calls to its three private MCP tools with per-tool
`approval_mode="approve"`; this is transport permission, not an Action approval.
The Agent cannot access the platform approval API. This uses the official
[Codex MCP configuration](https://learn.chatgpt.com/docs/config-file/config-reference).
Code Mode remains available to transport tool calls on CLI versions that use it;
it does not enable the disabled shell/browser/plugin tools.
