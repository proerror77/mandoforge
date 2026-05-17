#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.TENANT_ROUTING_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.TENANT_ROUTING_CONTROLLER_PORT || "18790");
const controllerToken = process.env.TENANT_ROUTING_CONTROLLER_TOKEN || "";
const apiBaseUrl = (process.env.TENANT_ROUTING_CONTROLLER_API_URL || "http://127.0.0.1:18787").replace(/\/$/, "");
const apiSubject = process.env.TENANT_ROUTING_CONTROLLER_API_SUBJECT || "whiskey-tenant-routing-controller";
const apiRoles = process.env.TENANT_ROUTING_CONTROLLER_API_ROLES || "admin";
const timeoutMs = Number(process.env.TENANT_ROUTING_CONTROLLER_TIMEOUT_MS || "5000");

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkAuth(request) {
  if (!controllerToken) {
    return true;
  }
  return request.headers.authorization === `Bearer ${controllerToken}`;
}

async function readJson(request) {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > 1024 * 1024) {
      throw new Error("request body too large");
    }
    chunks.push(chunk);
  }
  const raw = Buffer.concat(chunks).toString("utf8");
  return raw ? JSON.parse(raw) : {};
}

async function apiFetch(path) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(`${apiBaseUrl}${path}`, {
      signal: controller.signal,
      headers: {
        "x-mandoforge-subject": apiSubject,
        "x-mandoforge-roles": apiRoles,
      },
    });
    const text = await response.text();
    if (!response.ok) {
      throw new Error(`${path} returned HTTP ${response.status}: ${text.slice(0, 200)}`);
    }
    return text ? JSON.parse(text) : {};
  } finally {
    clearTimeout(timer);
  }
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function check(name, status, details = {}) {
  return { name, ...details, status };
}

function summarizeProductionBlockers(payload, liveReadiness) {
  const reasons = new Set([
    ...((payload.production_routing && payload.production_routing.blocking_reasons) || []),
    ...((liveReadiness.production_routing && liveReadiness.production_routing.blocking_reasons) || []),
  ]);
  return [...reasons];
}

function buildChecks(payload, liveReadiness) {
  const payloadCounts = payload.scoped_counts || {};
  const liveCounts = liveReadiness.scoped_counts || {};
  const payloadTableCoverage = payload.table_coverage || {};
  const liveTableCoverage = liveReadiness.table_coverage || [];
  const liveMissingRlsTables = liveTableCoverage
    .filter((table) => !table.rls_enabled || !table.rls_forced)
    .map((table) => table.table);
  const runtimeTenantMatches = payload.runtime_tenant_id === liveReadiness.runtime_tenant_id;
  const runtimeModeMatches = payload.runtime_tenant_mode === liveReadiness.runtime_tenant_mode;
  const scopedCountsMatch = sameJson(payloadCounts, liveCounts);
  const tableCoverageMatches =
    payloadTableCoverage.tracked_table_count === liveTableCoverage.length &&
    sameJson(payloadTableCoverage.missing_rls_tables || [], liveMissingRlsTables);
  const rls = liveReadiness.rls || {};
  const productionRouting = liveReadiness.production_routing || {};
  const rlsReady = Boolean(rls.enabled && rls.forced && rls.tenant_context_configured);
  const crossTenantRoutingSupported = liveReadiness.runtime_tenant_mode === "tenant_routed";

  const identityMatches = runtimeTenantMatches && runtimeModeMatches;
  const metadataMatches = scopedCountsMatch && tableCoverageMatches;

  return [
    check("api_health", "passed", { target: `${apiBaseUrl}/healthz` }),
    check("readiness_payload_matches_live_api", identityMatches ? (metadataMatches ? "passed" : "attention") : "failed", {
      runtime_tenant_matches: runtimeTenantMatches,
      runtime_mode_matches: runtimeModeMatches,
      scoped_counts_match: scopedCountsMatch,
      table_coverage_matches: tableCoverageMatches,
    }),
    check("header_fail_closed", liveReadiness.header_fail_closed ? "passed" : "failed"),
    check("membership_scope_enforced", liveReadiness.membership_scope_enforced ? "passed" : "failed"),
    check("runtime_routing", crossTenantRoutingSupported ? "passed" : "blocked", {
      runtime_tenant_mode: liveReadiness.runtime_tenant_mode,
    }),
    check("rls_context", rlsReady ? "passed" : "blocked", {
      enabled: Boolean(rls.enabled),
      forced: Boolean(rls.forced),
      tenant_context_configured: Boolean(rls.tenant_context_configured),
      rls_status: rls.status || "unknown",
    }),
    check("production_routing_readiness", productionRouting.production_blocked ? "blocked" : "passed", {
      readiness_status: productionRouting.status || "unknown",
    }),
  ];
}

const server = http.createServer(async (request, response) => {
  if (request.method === "GET" && request.url === "/healthz") {
    writeJson(response, 200, { status: "ok" });
    return;
  }
  if (request.method !== "POST" || request.url !== "/tenant/routing/validate") {
    writeJson(response, 404, { status: "not_found" });
    return;
  }
  if (!checkAuth(request)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }

  try {
    const payload = await readJson(request);
    if (payload.type !== "mandoforge.tenant_production_routing_validation") {
      writeJson(response, 422, {
        status: "failed",
        message: "unexpected validation payload type",
        checks: [check("payload_type", "failed", { observed: payload.type || null })],
      });
      return;
    }

    await apiFetch("/healthz");
    const liveReadiness = await apiFetch("/api/tenant-isolation/readiness");
    const checks = buildChecks(payload, liveReadiness);
    const hardFailed = checks.some((item) => item.status === "failed");
    const blockers = summarizeProductionBlockers(payload, liveReadiness);

    if (hardFailed) {
      writeJson(response, 502, {
        status: "failed",
        validation_id: `whiskey-tenant-routing-${Date.now()}`,
        message: "Whiskey tenant routing controller detected inconsistent or unsafe tenant evidence",
        checks,
        production_blockers: blockers,
      });
      return;
    }

    writeJson(response, 200, {
      status: "validated",
      validation_id: `whiskey-tenant-routing-${Date.now()}`,
      message: blockers.length === 0
        ? "Whiskey tenant routing target validated"
        : "Whiskey tenant routing controller validated live evidence collection; production routing remains blocked by reported readiness blockers",
      checks,
      production_blockers: blockers,
    });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey tenant routing validation failed",
      checks: [check("controller_execution", "failed", { target: apiBaseUrl })],
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Tenant routing controller listening on http://${listenHost}:${listenPort}`);
});
