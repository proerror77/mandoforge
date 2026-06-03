# Yew Frontend Replacement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the React/Vite/TanStack Query console with a Rust/Yew/Trunk WebAssembly Agent OS console that builds into `web/`.

**Architecture:** `web-ui/` becomes a standalone Yew client-side app. Trunk writes hashed static assets into the existing `web/` directory, so the Rust API server continues serving `ServeDir::new("web")` unchanged. The Yew app keeps polling-based API observability and uses typed serde structs plus a small fetch helper instead of a JavaScript query layer.

**Tech Stack:** Rust 1.96, Yew 0.23, Trunk 0.21, gloo-net, wasm-bindgen-futures, serde, web-sys.

---

### Task 1: Replace Build Stack

**Files:**
- Create: `web-ui/Cargo.toml`
- Create: `web-ui/Trunk.toml`
- Replace: `web-ui/index.html`
- Delete: `web-ui/package.json`
- Delete: `web-ui/package-lock.json`
- Delete: `web-ui/vite.config.ts`
- Delete: `web-ui/tsconfig.json`
- Delete: `web-ui/tsconfig.tsbuildinfo`
- Delete: `web-ui/src/*.tsx`
- Delete: `web-ui/src/*.ts`

- [x] Remove Node/Vite/React metadata.
- [x] Add Yew dependencies and Trunk output config.
- [x] Keep the page title `MandoForge Agent OS Console` and root mount point.

### Task 2: Implement Yew Data Layer

**Files:**
- Create: `web-ui/src/api.rs`

- [x] Define serde structs for agents, sessions, worker jobs, workflows, approvals, tool calls, task board, semantic graph, packs, deployment version, and readiness summaries.
- [x] Implement `api_get`, `api_post`, admin token storage, and JSON helpers using `gloo_net::http::Request`.
- [x] Keep literal `/api/...` strings in Rust source so `verify-ui-api-truth-gate.mjs` can enforce backend route coverage.

### Task 3: Implement Console UI

**Files:**
- Create: `web-ui/src/main.rs`
- Create: `web-ui/src/styles.css`

- [x] Build top navigation tabs: Agents, Board, Workflows, Dynamic, Semantic, Packs, Deploy.
- [x] Make Agents the observability-first home: running agents, worker state, approvals, tool calls, logs/artifacts, task launcher, and deployment version.
- [x] Implement focused pages for board, workflows/dynamic workflows, semantic memory, packs, and deploy.
- [x] Use polling via Yew hooks and `spawn_local`, not a global state library.

### Task 4: Update Gates And CI

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/deploy.yml`
- Modify: `scripts/verify-static-ui-assets.sh`
- Modify: `scripts/verify-ui-api-truth-gate.mjs`

- [x] Replace npm build steps with Rust/Trunk setup and `trunk build --release --dist ../web`.
- [x] Make static UI verification look for Trunk assets and Yew console text.
- [x] Make UI/API truth gate scan `web-ui/src/**/*.rs`.

### Task 5: Verify

**Commands:**
- `trunk build --release --dist ../web` from `web-ui/`
- `scripts/verify-static-ui-assets.sh`
- `node scripts/verify-ui-api-truth-gate.mjs`
- If API is running: `BASE_URL=http://127.0.0.1:8787 node scripts/verify-ui-api-truth-gate.mjs`

- [x] Fix compile errors and gate failures until all required checks pass or a real external blocker is identified.
