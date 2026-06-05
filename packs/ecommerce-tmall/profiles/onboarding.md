# Tmall Ecommerce Onboarding

Collect and normalize tenant-specific store truth before release.

Required onboarding outputs:

- `store`: typed store profile with identity, category, policy, escalation, and
  campaign fields.
- `approval-matrix`: approval owners for critical, high, medium, and low risk
  operations, plus workflow-specific approver groups.
- `connector-map`: operation-level TOP/QianNiu/media/product-edit readiness and
  degraded-lane mapping.
- `connector-account`: tenant, workspace, shop, seller, TOP session auth type,
  and secret reference names. Raw secret values must be stored by the runtime
  secret store, not in the pack profile.
- `ontology-seed`: ecommerce/Tmall object types, relation types, and retrieval
  defaults used to seed Context OS semantics for the tenant.
- `risk-policy`: tenant-specific examples of critical, high, medium, and low
  cases.
- `output-style`: buyer-facing tone, review-reply tone, internal-note tone, and
  content-production tone.

Release blockers:

- Store identity missing.
- No customer-service escalation owner.
- No after-sales/refund policy owner.
- No review/VOC policy owner.
- No content compliance owner.
- No approval owner for external writes.
- TOP connector credentials missing for every read lane.
- TOP connector account binding missing `shop_id`, `seller_nick`, or required
  secret references.
- No connector readiness report maps operation status to workflow lane impact.

Degraded but releasable:

- Comment/Q&A API unavailable but operator-import evidence is enabled.
- Media upload unavailable but content brief generation remains enabled.
- Product-edit permission unavailable but media association remains manual.
- QianNiu task creation unavailable but internal artifact handoff remains
  enabled.
- Optional write operations unavailable while draft-only and approval-gated
  workflow outputs remain usable.
