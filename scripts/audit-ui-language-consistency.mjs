#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const files = [
  "web-ui/src/api.rs",
  "web-ui/src/state.rs",
  "web-ui/src/main.rs",
  "web-ui/src/notifications.rs",
  "web-ui/src/views/overview.rs",
  "web-ui/src/views/agents.rs",
  "web-ui/src/views/workflows.rs",
  "web-ui/src/views/semantic.rs",
  "web-ui/src/views/packs.rs",
  "web-ui/src/views/deploy.rs",
  "web-ui/src/views/settings.rs",
].map((file) => path.join(root, file));

const required = [
  ["state.rs", "UiLang::En => \"en\""],
  ["state.rs", "View::Overview => \"Overview\""],
  ["state.rs", "View::Agents => \"Managed Agents\""],
  ["state.rs", "View::Workflows => \"Runs & Tasks\""],
  ["state.rs", "View::Semantic => \"Ontology\""],
  ["state.rs", "View::Packs => \"Capabilities\""],
  ["state.rs", "View::Deploy => \"System Ops\""],
  ["main.rs", "mandoforge.uiLang"],
  ["main.rs", "Console language"],
  ["semantic.rs", "type SemanticLang = UiLang"],
  ["main.rs", "matches!(*active_view, View::Workflows)"],
  ["main.rs", "fn use_polling<T>(path: &'static str, interval_ms: u32, enabled: bool)"],
  ["main.rs", "fn use_polling_dynamic<T>(path: String, interval_ms: u32, enabled: bool)"],
  ["api.rs", "DEFAULT_ONTOLOGY_DOMAIN_SCOPE"],
  ["settings.rs", "Console Defaults"],
];

const banned = [
  ["agents.rs", 'Panel title="Task launcher"'],
  ["agents.rs", 'Panel title="Runtime topology"'],
  ["agents.rs", 'Panel title="Queue pressure"'],
  ["agents.rs", 'Panel title="Worker state"'],
  ["packs.rs", 'Panel title="Marketplace map"'],
  ["packs.rs", 'Panel title="Installations"'],
  ["packs.rs", 'Panel title="Marketplace"'],
  ["deploy.rs", 'Panel title="Latest deployment"'],
  ["deploy.rs", 'Panel title="Enterprise product readiness"'],
  ["settings.rs", 'Panel title="Notification policy"'],
  ["settings.rs", 'Panel title="Desktop integration"'],
  ["semantic.rs", "use_state(|| SemanticLang::Zh)"],
  ["semantic.rs", "use_state(|| UiLang::Zh)"],
  ["api.rs", "x-mandoforge-roles"],
  ["api.rs", "x-mandoforge-subject"],
  ["main.rs", "auth-strip"],
  ["main.rs", "mandoforge-admin-token"],
  ["main.rs", 'domain_scope=ecommerce&workflow_scope=tmall'],
  ["main.rs", '"target": "whiskey"'],
  ["api.rs", '"domain_scope": "legal"'],
  ["api.rs", '"workflow_scope": "contract-review"'],
  ["api.rs", '"memory_scope": "legal-policy"'],
];

const byName = new Map(
  files.map((file) => [path.basename(file), fs.readFileSync(file, "utf8")]),
);

const failures = [];

for (const [name, needle] of required) {
  if (!byName.get(name)?.includes(needle)) {
    failures.push(`Missing required UI language marker in ${name}: ${needle}`);
  }
}

for (const [name, needle] of banned) {
  if (byName.get(name)?.includes(needle)) {
    failures.push(`Found stale mixed-language UI string in ${name}: ${needle}`);
  }
}

if (failures.length > 0) {
  console.error("UI language consistency audit failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("UI language consistency audit passed.");
