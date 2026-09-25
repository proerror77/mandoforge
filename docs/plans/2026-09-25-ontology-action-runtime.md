## Outcome

Make the existing business Agent runtime enforce a real Ontology-read-before-Action chain. Codex must obtain business facts and Action contracts through a scoped tool interface, submit registered Actions itself, and finish only after controlled execution and independent readback. This work does not adopt AX, merge PR #38, optimize unrelated prompt context, deploy production, or send external business messages.

## Acceptance criteria

- Expose an authenticated, session/TaskGrant-bound Ontology and Action tool path to real Codex CLI and the native worker. Do not provide full business facts/answers in the test prompt or have the driver execute the Agent's work.
- Server-issued read evidence binds current tenant, session, grant, release, object scope and versions. Skip-read, forged/mismatched/stale receipts and revoked/expired grants fail closed at proposal and execution boundaries.
- Reuse existing Ontology consumer, Action registration, policy, approval, commit-token, audit and idempotent execution mechanisms. Enumerate Agent business write entrypoints and prevent silent bypass. Health/transport handshakes are not business mutations.
- Preserve proposal/approval/execution/readback distinctions. High-risk and external actions stay approval-gated. No driver SQL writes to make acceptance pass.
- Use a registered internal draft Action for a synthetic customer-follow-up scenario. Include a controlled business completion/WorkItem closeout mechanism if the scenario requires closing it; an arbitrary approved Review is insufficient.
- A real model must read Ontology, propose/call the Action, and read its business result. Record tool order, identities, versions, permission/approval decisions, execution receipt and final state. Validate one full run before expanding concurrency.
- Negative tests cover skip-read, forgery/session/object mismatch, pre-approval side effects, grant revocation/expiry, duplicate execution and failed-Action completion rejection.
- Deliver reviewed code, focused tests, real runtime evidence, an independent draft PR and terminal CI for its current head. Do not merge or deploy production.

## Initial implementation scope

Extend the existing Ontology/TaskGrant and Action contracts with durable read receipts; provide a narrowly scoped runtime tool bridge for Codex using native supported CLI/MCP facilities; integrate it through the existing approved execution/worker path; add the minimal registered internal business Action and completion evidence needed by the example. Preserve the separate AX experiment. Refine exact file ownership after reading the implementation.

## Implementation boundary

- A trusted queued ToolCall/TaskGrant selects the business runtime. The Agent receives only a short-lived capability to a private loopback MCP endpoint bound to that exact session, grant, application and authority digest. It cannot use that capability at the general API or choose another session/grant. Child grants cannot remove or widen inherited business authority.
- Business Codex execution uses native MCP with generic shell/app/plugin/delegation surfaces disabled and a restricted child environment. User login remains in its existing supported location; platform DB/admin/worker secrets are not inherited. Unsupported business adapters fail explicitly.
- Immutable server read receipts (migration 0083) bind source ToolCall, tenant/session/grant/application/context/release, scoped object and relationship versions, and expiry. The same scope is checked at proposal and committed effects; client-provided statements that a read occurred are never evidence.
- The first registered internal Action creates a local follow-up draft, with explicit business approval. An internal closeout Action may close its bound WorkItem only when the approved draft effect and an Agent readback receipt exist and match. Closeout is a registered, narrowly authorized control action; it does not automatically approve any pending action and must obey its published policy.
- The existing atomic approval/claim store transaction is extended to include the local business effect. External executors remain proposal-only. Action and final business state readback remain distinct from model text or process exit.
- An Agent turn awaiting business approval ends without claiming business completion, releasing the session execution lease so the approved Action can run. A later Agent turn reads the persisted state and continues; no driver substitutes for business reasoning or Action submission.
- Real verification will use a synthetic customer and rule stored in Ontology. The prompt contains only the goal and task identity. For the authorized isolated fixture, an independent test Approver may decide through the normal approval API after a concrete proposal exists. This is test approval evidence, not human approval. The Agent has no approval capability; real customer/high-risk/external actions retain their existing approval requirements.

## Local acceptance evidence

On 2026-09-25, the opt-in real Codex CLI 0.155.1 / native worker / Postgres 16
fixture passed in approximately 84 seconds. The Agent made seven successful
native MCP calls across three model turns: three context reads, two Action
proposals and two result reads. Two independent test-role approvals produced one
internal draft and a closed WorkItem; the workflow completed and its session
terminated. The driver only registered initial data, started the workflow,
approved the two scoped proposals and inspected the result.

The API suite passes 670 tests, with 27 environment-dependent tests ignored in
that suite. Three Postgres business tests are run separately, including receipt
immutability/non-superuser RLS, stale-effect rejection, closeout and duplicate
effect idempotency. CI explicitly runs that Postgres subset. Real-provider tests
remain opt-in and reuse supported local login. No production or distributed-scale
claim follows from this local fixture.
