# Interactive Graph UI Islands Plan

Date: 2026-06-15

## Goal

Upgrade the confusing graph-heavy console surfaces into real interactive UI
experiences without rewriting the entire Yew console.

The immediate target is the Ontology page. It should feel like an inspectable
knowledge graph: pan, zoom, drag, select, filter, highlight neighbors, inspect
evidence, and approve proposals in batches.

The broader target is a reusable interaction pattern for other graph surfaces:

- Ontology / semantic relationship graph
- Workflow and dynamic plan DAGs
- Managed agent runtime topology
- Later, dependency and task-board graph views if needed

## Current State

- The console shell is Rust/Yew/Trunk under `web-ui/`.
- Trunk emits static assets into `web/`, and the API serves them through
  `ServeDir::new("web")`.
- The Content Security Policy in `crates/mandoforge-api/src/main.rs` allows
  self-hosted scripts but pins the generated Trunk bootstrap hash.
- The Ontology page currently uses a Yew/SVG topology implementation. It has
  node selection and batch approval, but it is not a true graph component.
- Existing product direction says Ontology should default to a relationship
  graph or mind-map, with JSON/details moved into advanced or inspector areas.
- Existing ontology-builder design requires the graph to answer whether objects,
  relations, metrics, actions, and mappings are correct before publication.

## Design Principles

1. Keep the Yew app as the product shell.
   The current shell, API wiring, auth headers, polling, routes, and generated
   static build path should stay intact.

2. Use JavaScript islands for interaction-heavy canvases.
   Yew should own business state and review actions. A JS island should own
   graph layout, pan/zoom, selection, dragging, and rendering performance.

3. Do not introduce CDN runtime dependencies.
   Desktop/offline use and CSP hardening require vendored static assets under
   `web-ui/public/vendor/`.

4. The graph is for judgment, not decoration.
   The primary screen must show relationships at a glance. Evidence belongs in
   an inspector, not as repeated cards under the graph.

5. Preserve the proposal-first ontology workflow.
   LLM output stays draft/proposal-only. Human approval materializes semantic
   objects, semantic links, and compiled tools.

## Library Decision

### Ontology graph: Cytoscape.js

Use Cytoscape.js first for Ontology because it is built for graph/network data:

- Nodes and edges are first-class.
- Pan, zoom, drag, select, box select, and neighbor highlighting are native.
- It supports force, concentric, breadth-first, and preset layouts.
- It maps naturally to object types, source tables, link types, actions,
  metrics, and compiled tools.
- It does not require moving the whole app to React.

### Workflow/DAG builder: React Flow or XYFlow later

Use React Flow/XYFlow later only for editable workflow diagrams where users
connect steps, edit node forms, and reason about DAG execution. It is better for
node-editor workflows than ontology knowledge graphs.

### Large graph projection: Sigma.js later

If enterprise ontology graphs become too large for Cytoscape in the browser,
add a separate Sigma/WebGL projection. This is not needed for the MVP review
graph.

## Architecture

```text
Yew shell
  -> route/page state
  -> API calls and review mutations
  -> inspector and approval controls
  -> passes bounded graph JSON to JS island

JS graph island
  -> mounts into a Yew NodeRef div
  -> owns Cytoscape instance
  -> renders graph with pan/zoom/drag/layout
  -> emits node/edge/selection events back to Yew
```

Suggested files:

- `web-ui/public/vendor/cytoscape.min.js`
- `web-ui/public/ontology-graph.js`
- `web-ui/src/graph_island.rs`
- `web-ui/src/views/semantic.rs`
- `web-ui/src/styles.css`
- `web-ui/index.html`
- `crates/mandoforge-api/src/main.rs`

The public JS wrapper should expose a small stable API:

```javascript
window.MandoForgeOntologyGraph.mount(element, graphData, callbacks)
window.MandoForgeOntologyGraph.update(instance, graphData)
window.MandoForgeOntologyGraph.destroy(instance)
```

Yew should call this through `wasm-bindgen` or `js_sys`, not through ad hoc DOM
string injection.

## Ontology UX Target

### First screen

The first screen should be the graph plus a focused inspector:

```text
left rail: run/source/filter summary
center: interactive knowledge graph
right inspector: selected node/edge evidence and review action
bottom strip: compact proposal queue and batch actions
```

Avoid repeating the same evidence blocks under the graph. The graph should
answer "what connects to what"; the inspector should answer "why should I trust
this proposal?"

### Graph node types

- Source dataset / raw table
- Business object
- Property / mapped field, shown only when expanded
- Link type / relation
- Metric
- Action
- Compiled agent tool
- Policy or risk marker, shown as a badge or edge label

### Graph interactions

MVP interactions:

- Pan and zoom
- Drag nodes
- Click node or edge to inspect
- Highlight first-degree neighbors
- Fit to graph
- Reset layout
- Filter by node type
- Filter by review status
- Search node by label
- Select visible/high-confidence/pending proposal nodes
- Batch approve selected
- Batch approve high-confidence
- Batch reject selected with reason placeholder

Second-pass interactions:

- Expand/collapse field-level mappings under an object
- Hide non-neighbor context
- Layout switch: force, concentric, source-to-tool pipeline
- Confidence threshold slider
- Review diff mode: before materialization vs after materialization
- Keyboard shortcuts for next/approve/reject

## Other Pages

### Runs & Tasks

Current Workflow, Dynamic Plan, and Board concepts should stay grouped under
Runs & Tasks. The interactive surface should become a DAG view:

- Nodes: run steps, tools, approvals, artifacts, failures
- Edges: dependency order from the workflow graph
- Use React Flow/XYFlow only if editing becomes necessary
- For read-only run topology, Cytoscape can be reused first

### Managed Agents

The Manager Agent remains an observer/advisor. The graph should show runtime
topology, not a business-process editor:

- Agents
- Current sessions
- Queue pressure
- Approvals
- Tool calls
- Failed or blocked runs

Use this graph to answer: "Which agents are active, blocked, risky, or waiting
for approval?"

### Capabilities

Do not make Capabilities another graph-heavy page yet. Its job is package
readiness:

- Installed packs
- Available connectors
- Tools unlocked by each pack
- Production boundaries and missing setup

Use compact dependency diagrams only inside individual pack details.

## Implementation Phases

### Phase 1: Plan and contract

- Add this plan.
- Define the graph island boundary and event contract.
- Keep current Yew/SVG implementation until the Cytoscape island is working.

Acceptance:

- The plan identifies all source files and verification commands.
- No product code behavior changes in this phase.

### Phase 2: Ontology graph island MVP

- Vendor Cytoscape locally under `web-ui/public/vendor/`.
- Add `web-ui/public/ontology-graph.js` wrapper.
- Add a Yew component that mounts into a `NodeRef`.
- Convert existing review graph data into Cytoscape elements.
- Preserve the existing inspector and approval logic in Yew.

Acceptance:

- Graph renders with pan/zoom/drag.
- Clicking a node updates the Yew inspector.
- Existing approve/reject/batch approve still works.
- No inline script expansion beyond the existing Trunk bootstrap pattern.

### Phase 3: Ontology review UX cleanup

- Remove duplicated evidence dumps from the main page.
- Keep evidence inside the inspector and compact proposal queue.
- Add type/status/search filters.
- Add selected-node and selected-subgraph batch actions.
- Add "fit", "reset layout", and "focus neighbors" controls.

Acceptance:

- A reviewer can approve the high-confidence ecommerce demo proposals without
  clicking 30+ individual cards.
- A reviewer can inspect why `raw_tmall.trade` maps to `Order` and how it links
  to `Customer`, `OrderLine`, `Refund`, and compiled commerce tools.
- The page is bilingual consistently: Chinese primary label with English
  technical term where helpful, not mixed randomly.

### Phase 4: Shared graph island abstraction

- Extract a reusable Yew-to-JS island helper.
- Keep ontology-specific mapping separate from generic graph lifecycle.
- Document how future graph pages should pass nodes, edges, callbacks, and
  layout options.

Acceptance:

- Ontology page no longer owns generic mount/update/destroy boilerplate.
- The next DAG/topology page can reuse the island without copying JS lifecycle
  code.

### Phase 5: Runs & Tasks topology

- Add read-only workflow/run DAG topology using the shared island.
- Show dependency status, blocked nodes, approvals, artifacts, and retries.
- Keep Workflow Templates and Dynamic Plans as subviews under Runs & Tasks.

Acceptance:

- The page explains execution order visually.
- Cycles or blocked dependencies are visible before reading JSON.

### Phase 6: Managed Agent topology

- Add observer-style agent topology.
- Show agents, active runs, queue pressure, approvals, and failed tasks.
- Do not turn Manager Agent into a top-level autonomous executor.

Acceptance:

- The page answers what the managed agent fleet is doing now.
- Risky actions remain approval-gated.

## Verification

Local code checks:

```bash
cargo check --manifest-path web-ui/Cargo.toml
cd web-ui && env -u NO_COLOR trunk build --release
cargo check --manifest-path crates/mandoforge-api/Cargo.toml
git diff --check
```

Static asset checks:

- Confirm `web/` contains the vendored JS and updated generated assets.
- Update the CSP bootstrap hash in `crates/mandoforge-api/src/main.rs` if Trunk
  changes the generated inline bootstrap.
- Confirm no CDN URLs are introduced.

Runtime checks:

```bash
MANDOFORGE_INSECURE_DEV_AUTH=1 \
MANDOFORGE_STORE_BACKEND=memory \
MANDOFORGE_EXECUTION_QUEUE_BACKEND=memory \
cargo run --manifest-path crates/mandoforge-api/Cargo.toml
```

Then verify in Chrome DevTools or the in-app browser at:

```text
http://127.0.0.1:8787/
```

Browser acceptance:

- Ontology graph renders nonblank.
- Pan, zoom, drag, and fit controls work.
- Node click updates the inspector.
- Edge click or neighbor focus explains the relation.
- Batch approve high-confidence changes proposal status.
- Console has no relevant errors.
- Initial layout has no severe node overlap.
- Mobile/narrow layout still exposes graph and inspector without text overlap.

## Risks

- Adding a JS library can weaken the CSP if loaded from CDN or inline scripts.
  Mitigation: vendor locally and load only self-hosted static assets.

- Yew and JS can fight over DOM ownership.
  Mitigation: the JS island owns only one container div; Yew owns surrounding
  state and controls.

- Cytoscape layouts can still overlap on dense graphs.
  Mitigation: cap initial graph, add filters, fit controls, neighbor focus, and
  layout reset.

- React Flow could bloat the stack if introduced too early.
  Mitigation: defer React Flow to editable workflow/DAG surfaces. Use Cytoscape
  first for read-only topology and ontology graph review.

## Commit Plan

1. `Document interactive graph UI island plan`
2. `Add ontology Cytoscape graph island`
3. `Simplify ontology review page around graph inspector`
4. `Extract reusable graph island lifecycle`
5. `Add Runs & Tasks topology view`
6. `Add Managed Agent topology view`

Each commit should include generated `web/` assets only when frontend behavior
changes, and should avoid committing unrelated `.superpowers/` artifacts.
