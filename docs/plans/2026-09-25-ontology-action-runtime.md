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
