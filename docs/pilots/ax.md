## Outcome
Implement an explicitly enabled, isolated Google AX pilot under the existing governed `agent_cli.exec` boundary. No production cutover, merge, cloud provisioning, model spend, or business sends.

## Acceptance criteria
- Pin upstream AX source and identify compatible Substrate prerequisites from current code.
- Submit a fixed harmless sandbox task, read AX state and independently read its result; Running alone is not completion.
- Preserve managed profile, session identity, approval, audit, event and artifact evidence; do not add a competing business controller.
- Bound execution and fail closed on identity mismatch, ambiguous submission, disabled pilot and unsupported operations. Verify cancellation/checkpoint support against actual upstream behavior.
- Test contracts separately from real AX/Substrate execution, archive an honest readback and document any infrastructure blocker.
- Deliver focused tests and a reviewable draft PR; do not merge or deploy production.

## Proposed scope
Opt-in `ax_pilot` managed CLI profile and a Rust pilot executable reusing existing dependencies; fixed no-network/no-business-write diagnostic command; local durable submission receipt and AX task identity bound to a MandoForge session; focused tests, live verification script and pilot documentation. No new HTTP API or production readiness lane.
