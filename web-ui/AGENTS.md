# Web UI Instructions

- UI production status must come from live API readbacks or explicit evidence artifacts. Do not hard-code readiness, completion, or customer-grade claims.
- Keep literal `/api/...` route strings in Rust source when adding backend-backed surfaces so `scripts/verify-ui-api-truth-gate.mjs` can verify route coverage.
- Admin or high-risk controls must mirror backend authorization and approval boundaries. The UI may expose actions, but it must not imply execution succeeded until the API confirms it.
- Avoid generated build artifacts in commits. Keep `web-ui/target/` and other local build outputs out of review scope.
- When changing UI API surfaces, run:
  - `cargo check --manifest-path web-ui/Cargo.toml`
  - `node scripts/verify-ui-api-truth-gate.mjs`

