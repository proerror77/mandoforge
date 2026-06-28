# API Instructions

- Keep production readiness fail-closed. Do not mark pilot, mock, local, or static-only evidence as customer-grade production evidence.
- When adding or changing an enterprise readiness lane, update the lane definition, user-facing remediation text, and the targeted readiness tests together.
- Readiness checks should distinguish static repo wiring from dynamic runtime proof. Static checks can show the gate exists; production-ready status requires the required evidence artifact or live runtime readback.
- Preserve tenant, permission, audit, and approval boundaries on write paths. High-risk business actions remain draft or approval-gated unless a production policy explicitly enables execution.
- Usual verification for API changes:
  - `cargo fmt --all -- --check`
  - `cargo check -p mandoforge-api --bins`
  - targeted `cargo test --manifest-path crates/mandoforge-api/Cargo.toml <filter> -- --nocapture` for the changed behavior

