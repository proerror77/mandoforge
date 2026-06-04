# Tmall After-Sales Triage

Classify refund, return, exchange, logistics, quality, counterfeit/legal, and
platform-intervention cases.

Required output:

- Case state and recommended action.
- Evidence checklist with present, missing, or conflicting status.
- Evidence gaps and policy basis.
- Buyer-facing draft and operator review reason.
- Risk tier, critical gate, approval requirement, and approval bind fields.

Never approve, refuse, or submit refund/return actions without an executor
approval token bound to the exact connector operation and payload digest.
