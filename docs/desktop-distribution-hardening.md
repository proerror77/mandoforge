# Desktop Distribution Hardening

MandoForge Desktop is an operator shell for an API-backed control plane. It does not own governance, approval, audit, connector readiness, ontology promotion, or enterprise completion truth.

## Current Contract

- The desktop crate opens an existing API-backed console from `MANDOFORGE_API_BASE_URL`, defaulting to `http://127.0.0.1:8787`.
- `MANDOFORGE_DESKTOP_EMBEDDED_API=1` can start a bounded local API process from `MANDOFORGE_DESKTOP_API_COMMAND` on a reserved localhost port.
- Single-instance behavior is enabled. A second launch focuses the existing `main` window instead of creating a duplicate shell.
- Critical web-console notifications can be forwarded to native OS notifications through `forward_console_notification`.
- Autostart is available only as explicit operator opt-in through `set_autostart_enabled`. It is not enabled silently or by default.
- `get_desktop_hardening_status` reports signed distribution, updater, CSP, and enterprise completion as false until their evidence exists.

## Not Production Distribution Yet

The Tauri bundle metadata exists, but `bundle.active` remains false. Before distributing a packaged app, the project still needs:

- Signed macOS, Windows, and Linux package evidence.
- A signed updater feed with public key, release metadata, rollback notes, and verification logs.
- Packaged WebView CSP review.
- Packaged notification permission evidence on each supported OS.
- Customer-grade runtime, connector, ontology, WorkflowPack lifecycle, and deployment evidence as defined in `docs/enterprise-product-completion-contract.md`.

## Autostart Policy

Autostart must stay operator controlled:

- Default state is off.
- The app must expose current OS registration state before changing it.
- Enabling or disabling autostart must go through an explicit command or UI action.
- Autostart must not imply customer-grade readiness, live connector production readiness, or enterprise completion.

## Verification

Use these gates for the desktop shell:

```bash
cargo check -p mandoforge-desktop --locked
cargo test -p mandoforge-desktop -- --test-threads=1
./scripts/verify-desktop-shell-contract.sh
./scripts/verify-desktop-runtime-smoke.sh
```

Use `EMBEDDED_API=1 ./scripts/verify-desktop-runtime-smoke.sh` to verify the bounded embedded-local API path.
