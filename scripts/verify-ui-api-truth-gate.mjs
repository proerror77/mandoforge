#!/usr/bin/env node
import fs from "node:fs";
import process from "node:process";

const BACKEND_ROOT = "crates/mandoforge-api/src";
const BACKEND_ENTRYPOINT = `${BACKEND_ROOT}/main.rs`;
const BACKEND_HANDLERS = `${BACKEND_ROOT}/handlers`;
const FRONTEND_ROOT = "web-ui/src";

const ACTION_EXPANSIONS = new Map([
  ["/api/approvals/{param}/{param}", ["/api/approvals/{param}/approve", "/api/approvals/{param}/reject"]],
  [
    "/api/memory-writeback-candidates/{param}/{param}",
    ["/api/memory-writeback-candidates/{param}/approve", "/api/memory-writeback-candidates/{param}/reject"],
  ],
  [
    "/api/agent-handoffs/{param}/{param}",
    [
      "/api/agent-handoffs/{param}/accept",
      "/api/agent-handoffs/{param}/reject",
      "/api/agent-handoffs/{param}/fail",
      "/api/agent-handoffs/{param}/complete",
      "/api/agent-handoffs/{param}/escalate",
    ],
  ],
]);

const LIVE_CHECKS = [
  ["GET", "/api/agents"],
  ["GET", "/api/environments"],
  ["GET", "/api/sessions"],
  ["GET", "/api/approvals"],
  ["GET", "/api/execution-jobs"],
  ["GET", "/api/session-loop-jobs"],
  ["GET", "/api/tool-calls"],
  ["GET", "/api/workflow-runs"],
  ["GET", "/api/workflow-definitions"],
  ["GET", "/api/task-board"],
  ["GET", "/api/work-items"],
  ["GET", "/api/manager-plans"],
  ["GET", "/api/agent-handoffs"],
  ["GET", "/api/agent-handoff-assignments"],
  ["GET", "/api/workflow-packs/installations"],
  ["GET", "/api/stage2/readiness"],
  ["GET", "/api/enterprise-product/readiness"],
  ["GET", "/api/enterprise-security/admin-readiness"],
  ["GET", "/api/native-connectors/production-readiness"],
  ["GET", "/api/observability"],
  ["GET", "/api/capability-discovery"],
  ["GET", "/api/usage"],
  ["GET", "/api/usage/finance-operations/summary"],
  ["GET", "/api/memory-governance/summary"],
  ["GET", "/api/memory-governance/writebacks?limit=50&status=pending"],
  ["GET", "/api/memory-writeback-candidates"],
  ["GET", "/api/scheduler/summary"],
  ["GET", "/api/deployment/version"],
  ["GET", "/api/remote-computers/production-path"],
  ["GET", "/api/workflow-packs/marketplace"],
  ["GET", "/api/semantic-objects"],
  ["GET", "/api/semantic-links"],
  ["GET", "/api/semantic-search"],
  ["GET", "/api/semantic-graph"],
  ["GET", "/api/semantic-workbench?domain_scope=legal"],
  ["GET", "/api/semantic-reflection/queue"],
  ["GET", "/api/ontology/registry"],
  ["GET", "/api/ontology/engine-readiness"],
  ["GET", "/api/semantic-retrieval/backends"],
  [
    "POST",
    "/api/semantic-ontology/builder",
    {
      domain_scope: "legal",
      workflow_scope: "contract-review",
      memory_scope: "legal-policy",
      objective: "Build a first-draft ontology proposal for legal contract review.",
      source_text: "Contract review uses Contract, Clause, Obligation, Risk, and Approval Requirement concepts.",
      source_refs: ["gate://semantic-ontology-builder"],
      max_object_types: 8,
      max_relation_types: 8,
      preview_only: true,
    },
  ],
];

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function listFrontendFiles(dir = FRONTEND_ROOT) {
  const files = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory()) {
      files.push(...listFrontendFiles(path));
    } else if (entry.isFile() && path.endsWith(".rs")) {
      files.push(path);
    }
  }
  return files.sort();
}

function listRustFiles(dir) {
  const files = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory()) {
      files.push(...listRustFiles(path));
    } else if (entry.isFile() && path.endsWith(".rs")) {
      files.push(path);
    }
  }
  return files.sort();
}

function backendRouteFiles() {
  const files = [BACKEND_ENTRYPOINT];
  if (fs.existsSync(BACKEND_HANDLERS)) {
    files.push(...listRustFiles(BACKEND_HANDLERS));
  }
  return [...new Set(files)].sort();
}

function backendRoutes() {
  const routes = [];
  for (const file of backendRouteFiles()) {
    for (const match of read(file).matchAll(/\.route\(\s*"([^"]+)"/gs)) {
      routes.push(match[1]);
    }
  }
  return [...new Set(routes)].sort();
}

function frontendApiRefs() {
  const refs = [];
  const quotedApi = /([`'"])(\/api\/[\s\S]*?)\1/g;
  for (const file of listFrontendFiles()) {
    const text = read(file);
    for (const match of text.matchAll(quotedApi)) {
      let path = match[2]
        .replace(/\$\{[^}]+\}/g, "{param}")
        .replace(/\{[^}]*\}/g, "{param}")
        .replace(/\?.*$/g, "");
      if (path.includes("\n")) {
        continue;
      }
      refs.push({ file, path });
    }
  }
  const seen = new Set();
  return refs.filter((ref) => {
    const key = `${ref.file}:${ref.path}`;
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

function routeRegex(path) {
  const escaped = path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`^${escaped.replace(/\\\{[^}]+\\\}/g, "[^/]+")}(?:\\?.*)?$`);
}

function assertStaticRoutes() {
  const routes = backendRoutes();
  const routePatterns = routes.map((route) => ({ route, pattern: routeRegex(route) }));
  const missing = [];

  for (const ref of frontendApiRefs()) {
    const expanded = ACTION_EXPANSIONS.get(ref.path) ?? [ref.path];
    for (const path of expanded) {
      if (!routePatterns.some(({ pattern }) => pattern.test(path))) {
        missing.push({ ...ref, expected: path });
      }
    }
  }

  if (missing.length > 0) {
    console.error("UI API refs without backend routes:");
    for (const item of missing) {
      console.error(`- ${item.file}: ${item.path} -> ${item.expected}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log(`static route gate passed: ${routes.length} backend routes, ${frontendApiRefs().length} UI API refs`);
}

async function assertLiveEndpoints(baseUrl) {
  const failures = [];
  for (const [method, path, body] of LIVE_CHECKS) {
    const response = await fetch(new URL(path, baseUrl), {
      method,
      headers: {
        "content-type": "application/json",
        "x-mandoforge-subject": "ui-truth-gate",
        "x-mandoforge-roles": "admin",
      },
      body: body ? JSON.stringify(body) : undefined,
    });
    const text = await response.text();
    if (!response.ok) {
      failures.push({ method, path, status: response.status, body: text.slice(0, 400) });
      continue;
    }
    if (path === "/api/semantic-ontology/builder") {
      const payload = JSON.parse(text);
      if (payload?.status !== "preview" || payload?.builder?.authority !== "proposal_only") {
        failures.push({ method, path, status: response.status, body: `unexpected builder response ${text.slice(0, 200)}` });
      }
    }
  }

  if (failures.length > 0) {
    console.error(`live endpoint gate failed against ${baseUrl}:`);
    for (const failure of failures) {
      console.error(`- ${failure.method} ${failure.path} -> ${failure.status}: ${failure.body}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log(`live endpoint gate passed against ${baseUrl}: ${LIVE_CHECKS.length} checks`);
}

assertStaticRoutes();

if (process.env.BASE_URL) {
  await assertLiveEndpoints(process.env.BASE_URL);
} else {
  console.log("set BASE_URL=http://127.0.0.1:8787 to run live endpoint checks");
}
