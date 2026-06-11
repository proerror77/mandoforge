# OpenFang-Inspired Dashboard And Desktop Productization Plan

## Requirements Summary

This plan turns the OpenFang comparison into an executable MandoForge productization path.

The correction is:

- MandoForge should not copy OpenFang's single-binary local Agent OS positioning.
- MandoForge should learn from OpenFang's product surface: dashboard information architecture, first-run wizard, desktop shell, tray/notification loop, and capability cards.
- MandoForge's durable position remains an enterprise Agent OS control plane: governed runtime, WorkflowPack lifecycle, ontology, approval, audit, connector readiness, release gates, rollback, and enterprise evidence.
- Desktop is a distribution and operator-access layer. It is not proof of enterprise product completion by itself.

Target outcome:

- A clearer, more product-complete control console.
- A first-run enterprise onboarding path.
- Pack cards that answer "what can this pack do now?"
- A desktop MVP that can open or start the local control plane and surface critical operator notifications.
- Verification gates that prevent UI polish from overstating runtime readiness.

## Evidence Baseline

Current MandoForge facts:

- `README.md:7-18` defines MandoForge as an Agent middleware platform and Agent OS kernel, not a vertical agent product.
- `README.md:89-97` explicitly says the repo is not a finished vertical SaaS product and not a complete production platform yet.
- `web-ui/src/main.rs:10-19` has seven current views: Agents, Board, Workflows, Dynamic, Semantic, Packs, and Deploy.
- `web-ui/src/main.rs:76-116` centralizes a large `ConsoleData` surface.
- `web-ui/src/main.rs:142-226` already polls real runtime, workflow, pack, connector, semantic, ontology, deployment, and readiness APIs.
- `web-ui/Trunk.toml:1-4` builds the Yew UI to `../web`.
- `crates/mandoforge-api/src/main.rs:7565` serves the static UI from `web`.
- `docs/enterprise-product-completion-contract.md:12-26` defines the enterprise completion rule and keeps pilot readiness separate from enterprise completion.
- `docs/enterprise-product-completion-contract.md:100-138` defines live connector production evidence, including sandbox/live separation, OAuth, retries, idempotency, reconciliation, and secret safety.
- `docs/enterprise-product-completion-contract.md:139-172` defines the ontology engine completion boundary.
- `docs/enterprise-product-completion-contract.md:174-197` defines WorkflowPack enterprise lifecycle requirements.
- `docs/workflow-pack-manifest-contract.md:81-156` defines install, update, stage, onboarding, connector quality, release, rollback, and archive lifecycle semantics.

Current OpenFang reference observations from `/tmp/openfang-inspect`:

- `crates/openfang-desktop/src/lib.rs:43-55` boots the embedded server and initializes desktop plugins.
- `crates/openfang-desktop/src/lib.rs:88-122` stores port/kernel state and opens a Tauri WebView against the local server.
- `crates/openfang-desktop/src/lib.rs:124-170` maps selected critical runtime events to native notifications.
- `crates/openfang-desktop/src/server.rs:61-111` binds `127.0.0.1:0`, starts the API server on a background thread, and returns a known port before the window opens.
- `crates/openfang-desktop/src/commands.rs:9-170` exposes IPC commands for status, port, import, autostart, update, and opening config/log directories.
- `crates/openfang-desktop/tauri.conf.json:1-59` packages the desktop app, updater, CSP, icons, and platform bundle metadata.
- `crates/openfang-api/static/js/pages/*` is organized into product pages such as overview, hands, channels, runtime, approvals, settings, wizard, sessions, usage, and workflows.

## Design Principles

1. Productize without changing the platform identity.
   The console should feel complete, but its claims must stay anchored to actual readiness APIs and evidence gates.

2. Keep Yew/Trunk for the current web console.
   Do not migrate to OpenFang's static JS/Alpine style. The existing backend already serves `web`, and the current UI already has typed API models.

3. Make the first screen operational.
   The default view should show active sessions, pending approvals, blocked lanes, pack readiness, connector readiness, ontology health, and worker queue health.

4. Treat packs as enterprise products.
   A pack card must show lifecycle state, workflows, connectors, approval gates, onboarding status, quality blockers, release status, rollback status, and demo entrypoints.

5. Desktop wraps the control plane.
   Desktop should launch, connect, focus, notify, and open logs/config. It should not own governance rules independently from the API.

6. Every visible readiness label must be backed by a route, script, audit event, or documented evidence class.

## What To Learn From OpenFang

Adopt these patterns:

- Modular dashboard pages with clear operator jobs.
- Overview page as the product home, not a dense debug console.
- First-run wizard that guides setup before the user hits an empty dashboard.
- Capability cards that package scenario value in human-facing terms.
- Desktop shell using Tauri 2, random localhost port, WebView to the same web app, tray menu, single-instance behavior, status IPC, open logs/config actions, and native notifications for critical events.
- Notification triage: only critical, actionable events should become OS notifications.

Do not copy these patterns directly:

- Do not make MandoForge a local-only single-binary runtime if that weakens enterprise deployment, tenant isolation, and evidence collection.
- Do not use "Hands" as a replacement for WorkflowPack lifecycle. MandoForge packs need install, stage, onboarding, release, rollback, archive, update, evidence, and tenant-specific connector quality.
- Do not treat desktop packaging, autostart, or updater as enterprise completion.
- Do not bypass existing approval, audit, policy, ontology, or connector readiness gates for desktop convenience.

## Current MandoForge Gaps

Product surface gaps:

- The UI has real data, but the information architecture is too dense for first-time enterprise evaluation.
- There is no first-run wizard that turns runtime primitives into a guided setup path.
- Packs do not yet read like "available enterprise capabilities" with current readiness and blockers.
- Notifications are not presented as a product-level operator loop.
- There is no desktop package or local app shell.

Architecture and readiness gaps:

- Enterprise completion still requires customer-grade evidence, not only repo-controlled validation.
- Live connectors must prove platform-specific semantics beyond generic approval gating.
- Ontology is ready as a foundation, but enterprise completion requires lifecycle, migration, review, enforcement, and operator-facing promotion flows.
- WorkflowPack lifecycle must be surfaced as product operations, not only manifest validation.

## Target Product Shape

Positioning:

```text
MandoForge = Enterprise Agent OS Control Plane
```

Top-level console IA:

- Overview: health, blockers, approvals, active runs, ready packs, connector readiness, ontology readiness, worker health.
- Runs: sessions, workflow runs, execution jobs, session-loop jobs, tool calls, artifacts, audit timeline.
- Approvals: pending approval queue, risk summary, affected workflow/pack, approve/reject history.
- Packs: marketplace cards, installed versions, lifecycle state, onboarding, connector quality, release, rollback, archive.
- Connectors: native/MCP connector production readiness, sandbox/live state, credentials, rate/error taxonomy, reconciliation, webhook/polling.
- Ontology: registry, engine readiness, semantic objects, semantic graph, context packet health, trust/freshness blockers.
- Environments: agents, environments, remote computer, workspace/runtime profiles.
- Readiness: enterprise product lanes, evidence freshness, gate scripts, blocked/pilot/customer-grade status.
- Settings: admin token/local mode, provider profiles, desktop integration, logs/config links.

Desktop shape:

- `crates/mandoforge-desktop` Tauri 2 shell.
- Start embedded local API or connect to an already running API.
- Open the existing web console in a WebView.
- Expose IPC commands for status, port/base URL, open logs, open config, open browser, and desktop notification permission/status.
- Tray menu: Show Console, Open Browser, Copy API URL, Open Logs, Status, Quit.
- Notify on pending approval, failed execution job, connector blocked, ontology readiness regression, enterprise readiness regression, and worker queue stuck.

## Implementation Plan

### Phase 1: Console Information Architecture Refactor

Goal:

Split the current large `web-ui/src/main.rs` into stable modules without changing behavior.

Proposed files:

- `web-ui/src/main.rs`
- `web-ui/src/state.rs`
- `web-ui/src/views/mod.rs`
- `web-ui/src/views/overview.rs`
- `web-ui/src/views/runs.rs`
- `web-ui/src/views/approvals.rs`
- `web-ui/src/views/packs.rs`
- `web-ui/src/views/connectors.rs`
- `web-ui/src/views/ontology.rs`
- `web-ui/src/views/environments.rs`
- `web-ui/src/views/readiness.rs`
- `web-ui/src/views/settings.rs`
- `web-ui/src/components/status.rs`
- `web-ui/src/components/cards.rs`
- `web-ui/src/components/tables.rs`

Rules:

- Keep existing API polling contracts.
- Keep the `web-ui/Trunk.toml` output to `../web`.
- Keep backend static serving through `ServeDir::new("web")`.
- Do not redesign data models during this phase.

Acceptance criteria:

- `web-ui/src/main.rs` is reduced to app shell, routing, state wiring, and top-level layout.
- Existing views remain reachable.
- No API endpoint currently polled by `ConsoleData` disappears from the UI.
- `NO_COLOR=false trunk build --release` passes from `web-ui/`.
- `./scripts/verify-static-ui-assets.sh` passes.
- `node scripts/verify-ui-api-truth-gate.mjs` passes.
- `git diff --check` passes.

### Phase 2: Product Overview Dashboard

Goal:

Make Overview the default operator home.

Use existing data sources:

- `/api/sessions`
- `/api/approvals`
- `/api/execution-jobs`
- `/api/session-loop-jobs`
- `/api/workflow-runs`
- `/api/workflow-packs/installations`
- `/api/workflow-packs/marketplace`
- `/api/enterprise-product/readiness`
- `/api/native-connectors/production-readiness`
- `/api/ontology/engine-readiness`
- `/api/observability`
- `/api/scheduler/summary`

Required widgets:

- Active managed sessions.
- Pending approvals.
- Failed/stuck execution jobs.
- Running workflow runs.
- Ready/blocked packs.
- Connector production readiness.
- Ontology readiness.
- Enterprise lanes: ready, pilot, blocked, stale evidence.
- Recent audit or observability highlights.

Acceptance criteria:

- The default route opens Overview.
- Every health badge links to the underlying detailed view.
- Each "blocked" label includes the blocker source.
- No generated or decorative status is shown without API backing.
- Browser validation confirms the dashboard renders at desktop and mobile widths without overlapping text.

### Phase 3: First-Run Enterprise Wizard

Goal:

Add a guided setup path that converts an empty or local install into a governed pilot.

Wizard steps:

1. Access mode: local dev, repo-controlled pilot, or customer-grade target.
2. Admin token and identity posture.
3. Provider/runtime profile.
4. Environment and workspace profile.
5. Choose first DomainPack or WorkflowPack.
6. Connector readiness check.
7. Ontology readiness check.
8. Create first governed session or workflow run.
9. Show final evidence checklist and remaining blockers.

Rules:

- Store progress locally until backend persistence is needed.
- Do not mark a pack ready if onboarding, connector quality, release, or approval gates are blocked.
- Do not create external writes during wizard flow.
- High-risk business actions remain draft/approval-only.

Acceptance criteria:

- Wizard can be opened from empty state and Settings.
- Wizard can be skipped without breaking the dashboard.
- Wizard completion creates or links to a governed session/workflow only through existing API policy boundaries.
- The final step shows exact remaining blockers from readiness endpoints.

### Phase 4: Pack Marketplace Cards

Goal:

Turn packs into understandable enterprise capability packages.

Initial cards:

- Ecommerce Ops.
- Tmall/Taobao Commerce.
- Xiaohongshu Shop.
- TikTok Shop.
- Amazon SP-API.
- Legal Review.
- AI Governance.

Each card must show:

- Pack kind and version.
- Main workflows.
- Required connectors.
- Data quality requirements.
- Approval gates.
- Current installation lifecycle state.
- Onboarding status.
- Connector quality status.
- Release gate status.
- Rollback/archive availability.
- Safe demo action.
- Blockers and evidence freshness.

Acceptance criteria:

- Pack cards are backed by marketplace and installation APIs.
- Cards make it clear whether a pack is only available, installed, staged, released, rolled back, archived, or blocked.
- Demo actions cannot imply live external writes unless live connector production evidence and approval binding exist.
- Existing lifecycle semantics from `docs/workflow-pack-manifest-contract.md` are visible in the UI.

### Phase 5: Notification Bridge

Goal:

Create a product notification loop before desktop packaging.

Web notification sources:

- Pending approvals.
- Failed execution jobs.
- Stuck session-loop jobs.
- Connector production readiness blockers.
- Ontology readiness blockers.
- Enterprise readiness regression.

Implementation shape:

- Add a small notification aggregator in `web-ui/src/notifications.rs`.
- De-duplicate events by stable key.
- Show in-console toasts or notification center first.
- Add optional browser notifications only for critical actionable items.
- Later map the same event classification into desktop OS notifications.

Acceptance criteria:

- Notification severity and action links are deterministic.
- No noisy polling-only repeat notifications.
- Approval and failed job notifications link to the exact detailed view.
- Critical notifications can be disabled in Settings.

### Phase 6: Desktop MVP

Goal:

Add a desktop wrapper that productizes local operation without forking governance.

Proposed files:

- `crates/mandoforge-desktop/Cargo.toml`
- `crates/mandoforge-desktop/src/main.rs`
- `crates/mandoforge-desktop/src/lib.rs`
- `crates/mandoforge-desktop/src/server.rs`
- `crates/mandoforge-desktop/src/commands.rs`
- `crates/mandoforge-desktop/src/tray.rs`
- `crates/mandoforge-desktop/src/notifications.rs`
- `crates/mandoforge-desktop/tauri.conf.json`
- `crates/mandoforge-desktop/capabilities/default.json`

Desktop behavior:

- If `MANDOFORGE_API_BASE_URL` is set, connect to that API.
- Otherwise start an embedded local API on `127.0.0.1:0`.
- Wait until the port/base URL is known before creating the WebView.
- Open the existing `web` console.
- Use the same API permissions and policy gates as the browser console.
- Provide IPC:
  - `get_status`
  - `get_api_base_url`
  - `open_browser`
  - `open_config_dir`
  - `open_logs_dir`
  - `get_notification_status`
  - `forward_console_notification`
  - `get_desktop_hardening_status`
  - `get_autostart_status`
  - `set_autostart_enabled`
- Provide tray:
  - Show Console
  - Open Browser
  - Copy API URL
  - Status
  - Open Logs
  - Quit

Desktop non-goals for MVP:

- No automatic live connector credential setup.
- No silent external writes.
- No independent pack release path.
- No enterprise completion claim.
- No production updater claim until signing and release metadata are implemented.

Acceptance criteria:

- `cargo check -p mandoforge-desktop` passes.
- Desktop can open the existing console against a running API.
- Desktop can start an embedded API in local mode if that is implemented in a bounded way.
- IPC status returns API URL, uptime, and connection state.
- Tray actions work on macOS at minimum.
- OS notifications fire only for critical events from the notification bridge.
- Desktop docs clearly label MVP boundaries.

### Phase 7: Desktop Hardening And Distribution

Goal:

Move from local desktop MVP to distributable app.

Required work:

- App icon and product metadata.
- macOS bundle profile.
- Windows bundle profile.
- Linux AppImage/deb profile.
- Signed update feed design.
- Log/config directory conventions.
- Single-instance behavior.
- Autostart only after explicit user opt-in.
- CSP review.
- Packaging verification script.

Acceptance criteria:

- `scripts/verify-desktop-shell-contract.sh` exists and validates desktop config, expected IPC commands, bundle metadata, and critical files.
- Packaging instructions are documented.
- Update path is either implemented with signing or explicitly marked unavailable.
- Single-instance behavior focuses the existing console instead of launching duplicate shells.
- Autostart remains disabled by default and can only be changed through an explicit operator command.
- Desktop still uses API-backed readiness and does not introduce local-only truth.

### Phase 8: Enterprise Product Completion Tie-In

Goal:

Make the productized dashboard drive enterprise completion instead of hiding gaps.

Required UI surfaces:

- Runtime production lane.
- Remote computer multi-node lane.
- Live connector production lane.
- Ontology engine lane.
- WorkflowPack enterprise lifecycle lane.
- Enterprise security/admin lane.
- Observability/support lane.
- Billing/cost/governance lane if already represented by readiness APIs.

Acceptance criteria:

- Readiness view reflects `docs/enterprise-product-completion-contract.md`.
- Each lane shows evidence class: repo-controlled, production-like pilot, or customer-grade.
- Each lane shows freshness.
- Each lane links to route/script/docs surfaces.
- Product UI cannot label the platform "enterprise complete" unless all required lanes are customer-grade ready.

## Verification Plan

Per UI phase:

```bash
cd web-ui
NO_COLOR=false trunk build --release
cd ..
./scripts/verify-static-ui-assets.sh
node scripts/verify-ui-api-truth-gate.mjs
git diff --check
```

Per backend/API-touching phase:

```bash
cargo fmt
cargo check -p mandoforge-api
./scripts/enterprise-product-readiness-gate.sh
./scripts/native-connector-production-readiness-gate.sh
./scripts/ontology-engine-readiness-gate.sh
./scripts/workflow-pack-evidence-gate.sh
git diff --check
```

Per desktop phase:

```bash
cargo fmt
cargo check -p mandoforge-desktop
./scripts/verify-static-ui-assets.sh
./scripts/verify-desktop-shell-contract.sh
git diff --check
```

Browser/UI validation:

- Run the local API and serve the built UI.
- Validate desktop viewport around `1440x900`.
- Validate mobile viewport around `390x844`.
- Check Overview, Packs, Approvals, Ontology, Readiness, and Settings.
- Confirm no text overlap, no blank screen, and no misleading ready labels.

Desktop validation:

- Launch desktop against existing API.
- Launch desktop in local embedded mode if enabled.
- Confirm WebView opens the same console.
- Confirm tray actions.
- Confirm open logs/config actions.
- Confirm notification de-duplication.
- Confirm quitting shuts down local embedded API cleanly when the desktop owns it.

## Risks And Mitigations

Risk: Dashboard becomes a marketing shell.

Mitigation: Every badge must cite or link to route-backed data. Keep readiness tied to existing evidence contracts.

Risk: Desktop duplicates API governance logic.

Mitigation: Desktop owns shell concerns only. API owns policy, approval, audit, pack lifecycle, connector readiness, and ontology enforcement.

Risk: OpenFang's local-product strengths pull MandoForge away from enterprise deployment.

Mitigation: Keep customer-grade enterprise lanes visible and blocked until proven. Treat desktop as an operator shell.

Risk: UI refactor breaks current real API wiring.

Mitigation: Phase 1 is behavior-preserving. Existing `ConsoleData` endpoints must stay visible or intentionally remapped.

Risk: Notifications become noisy.

Mitigation: Deduplicate by stable event key, notify only on state transitions, and keep browser/OS notification opt-in.

Risk: Desktop packaging creates false production confidence.

Mitigation: Desktop MVP explicitly excludes signed updater and enterprise completion claims until distribution evidence exists.

## Suggested Execution Order

1. Create Overview route and make the current Agents view no longer the default.
2. Extract UI state and view modules without behavior changes.
3. Build Overview dashboard from existing polling data.
4. Add Pack product cards and lifecycle status.
5. Add Readiness lane view tied to the enterprise completion contract.
6. Add first-run wizard.
7. Add notification aggregator and in-console notification center.
8. Add desktop crate with connect-to-existing-API mode first.
9. Add embedded-local-API mode only after the API startup contract is bounded.
10. Add tray, IPC, logs/config actions.
11. Add OS notifications using the same notification bridge.
12. Add desktop verification script.
13. Add packaging docs and signing/updater decision record.

## Execution Progress

- Phase 1 modularization slice implemented: `View`, `ConsoleData`, localStorage bootstrapping, reusable display components, and web notification classification were extracted into `web-ui/src/state.rs`, `web-ui/src/components.rs`, and `web-ui/src/notifications.rs` without changing the API polling contract or the Wizard/Overview runtime behavior.
- Phase 1 Wizard view extraction implemented: the First-run Enterprise Wizard now lives under `web-ui/src/views/wizard.rs`, with `web-ui/src/main.rs` retaining only the top-level route wiring for that view.
- Phase 1 Overview view extraction implemented: the default product home now lives under `web-ui/src/views/overview.rs`; `browser-harness` validated the Overview route at desktop and 390px mobile widths with 6 signal cards, 6 dashboard panels, and no horizontal overflow.
- Phase 1 Board view extraction implemented: the task board now lives under `web-ui/src/views/board.rs`; `browser-harness` validated the Board route at desktop and 390px mobile widths with 6 kanban columns, 2 panels, and no horizontal overflow.
- Phase 1 Workflows view extraction implemented: the workflow graph console now lives under `web-ui/src/views/workflows.rs`; `browser-harness` validated the Workflows route at desktop and 390px mobile widths with 5 panels, 3 workflow flow meters, and no horizontal overflow.
- Phase 1 Settings view extraction implemented: the operator settings surface now lives under `web-ui/src/views/settings.rs`, with `web-ui/src/main.rs` retaining only route wiring for Settings.
- Phase 1 Deploy view extraction implemented: the deployment/readiness surface now lives under `web-ui/src/views/deploy.rs`, with `web-ui/src/main.rs` retaining only route wiring for Deploy.
- Phase 1 Agents view extraction implemented: the managed agent observability surface now lives under `web-ui/src/views/agents.rs`; `browser-harness` validated the Agents route at desktop and 390px mobile widths with 10 panels and no horizontal overflow.
- Phase 1 boundary: business views still live in `web-ui/src/main.rs`; further extraction should move view-specific code into `web-ui/src/views/*` in small behavior-preserving commits.
- Phase 2 implemented: Overview is the product home, backed by live console APIs, and avoids the visual command deck pushing first-screen operator signals down.
- Phase 4 implemented: pack marketplace cards now expose lifecycle, workflows, agents, connectors, actions, release gates, files, connector gate copy, approval posture, and semantic scope from API-backed manifest summaries.
- Phase 5 implemented for web console: notification center classifies pending approvals, failed execution jobs, session-loop failures, connector readiness blockers, ontology blockers, and enterprise readiness regressions with stable keys and deterministic target views.
- Phase 5 boundary: browser-native permission prompting remains unimplemented; desktop OS notification forwarding is now mapped through the same critical notification bridge into Tauri with key-based de-duplication.
- Settings slice implemented: critical notification visibility can be muted per browser through `mandoforge.criticalNotificationsMuted`.
- Phase 6 initial MVP implemented: `crates/mandoforge-desktop` is a Tauri 2 shell that opens an existing API-backed console from `MANDOFORGE_API_BASE_URL` or `http://127.0.0.1:8787`, exposes status/base-url/logs/config/browser/notification IPC commands, and installs a macOS-capable tray menu.
- Phase 6 boundary: signed distribution and updater are not implemented yet; embedded API startup and native OS notification forwarding are implemented as bounded local-shell contracts, and `scripts/verify-desktop-shell-contract.sh` keeps those claims explicit.
- Phase 6 runtime smoke implemented: desktop status now reports API reachability (`api_reachable`, `api_unreachable`, or `api_url_invalid`), `MANDOFORGE_DESKTOP_SMOKE_EXIT_AFTER_MS` lets the Tauri event loop auto-exit for verification, and `scripts/verify-desktop-runtime-smoke.sh` can launch against an existing API or start a memory-mode API with `START_API=1`.
- Phase 6 runtime smoke passed after replacing the invalid placeholder icon with a Tauri-decodable PNG and adding `core:webview:allow-create-webview-window` to the desktop capability file.
- Phase 6 embedded local API implemented as an explicit opt-in process-owner contract: `MANDOFORGE_DESKTOP_EMBEDDED_API=1` starts an API command from `MANDOFORGE_DESKTOP_API_COMMAND` on a reserved localhost port, waits for API reachability before opening the WebView, and kills the owned child process on desktop exit.
- Phase 6 embedded boundary: this is not yet a Tauri packaged sidecar or signed distribution artifact; it is a verified local startup contract. `EMBEDDED_API=1 ./scripts/verify-desktop-runtime-smoke.sh` covers the opt-in path.
- Phase 7 hardening truth surface implemented: desktop IPC now exposes `get_desktop_hardening_status` with explicit false values for signed distribution, updater, CSP, and enterprise completion claims, while native notifications are reported as enabled only for critical events forwarded from the web notification bridge.
- Phase 7 single-instance and autostart slice implemented: the desktop shell registers Tauri single-instance behavior that refocuses the existing console, plus explicit opt-in `get_autostart_status` / `set_autostart_enabled` IPC backed by the OS autostart plugin; autostart remains disabled unless an operator enables it.
- Phase 7 desktop settings bridge implemented: the web settings view now detects the Tauri bridge, reports desktop status, single-instance/native-notification/autostart hardening state, and exposes explicit opt-in autostart enable/disable actions while remaining a safe browser no-op outside Tauri.
- Phase 7 boundary: the hardening surface is a truthful contract and verification hook, not implementation of signed packaging, updater, CSP, or packaged notification permission evidence.
- Phase 3 initial wizard slice implemented: `Wizard` is now a first-run console route with local/repo-pilot/customer-grade access modes, local progress storage, pack selection, identity/runtime/connector/ontology/evidence checks, and a governed pilot session launcher through the existing `/api/sessions` policy boundary.
- Phase 3 boundary: the wizard does not configure real external connector credentials, does not perform live writes, and does not mark customer-grade completion. It surfaces the current blockers from readiness APIs and links operators back to Packs, Deploy, Semantic, and Agents.
- Customer-grade evidence closure slice implemented in Deploy: the page now exposes a conservative checklist for real platform credentials, token refresh, reconciliation/idempotency, webhook/polling delivery, compensation policy, and archived deployment evidence, sourced from current readiness/connector/deployment JSON and marked blocked unless evidence is present.
- Live connector evidence gate tightened: `/api/native-connectors/production-readiness` now treats archived deployment evidence as a first-class customer-grade requirement per ecommerce connector and only reports `current_evidence_class=customer_grade` when every production-readiness check is present.

## Definition Of Done

This plan is complete when:

- The web console has a product-grade Overview, Packs, Readiness, and Wizard flow.
- Pack capability cards can answer "what this pack can do now" and "what blocks live use".
- Critical operator events are visible in the console and optionally forwarded to desktop notifications.
- Desktop opens or starts the control plane without bypassing governance.
- Enterprise product completion remains truthfully blocked until the completion contract lanes are customer-grade ready.
