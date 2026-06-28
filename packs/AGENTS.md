# Pack Instructions

- Workflow packs and connector manifests are part of the product contract. Keep schemas explicit and avoid hidden behavior that only exists in prose.
- Production connector manifests must declare `production_readiness` with `required_evidence_class: customer_grade` and `fail_closed_without_evidence: true`.
- Connector production readiness must cover token lifecycle, rate limits and retry policy, idempotency or reconciliation, webhook provenance, compensation policy, approval boundaries, secret redaction, and prompt-injection boundaries.
- Do not treat a demo connector, local fixture, or pilot transcript as production-ready evidence.
- When changing packs or connector manifests, run `scripts/verify-workflow-pack-manifest.sh`. If production connector semantics changed, also run the relevant production semantics gate.

