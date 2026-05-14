#!/usr/bin/env node
"use strict";

const http = require("http");

const port = Number(process.env.STAGE2_MOCK_CONTROLLER_PORT || "18080");
const host = process.env.STAGE2_MOCK_CONTROLLER_HOST || "127.0.0.1";

function readBody(request) {
  return new Promise((resolve, reject) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
      if (body.length > 2_000_000) {
        reject(new Error("request body too large"));
        request.destroy();
      }
    });
    request.on("end", () => {
      if (!body.trim()) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(body));
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });
}

function responseFor(pathname, payload) {
  const base = {
    message: "stage2 mock controller accepted evidence",
    received_type: payload.type || null,
    steps: [{ name: "mock-controller", status: "ok" }],
  };

  if (pathname.includes("/provider/rollout/rollback")) {
    return { ...base, status: "rolled_back", rollback_id: "mock-provider-rollback" };
  }
  if (pathname.includes("/provider/rollout/apply")) {
    return { ...base, status: "applied", deployment_id: "mock-provider-rollout" };
  }
  if (pathname.includes("/mcp/rollout/rollback")) {
    return { ...base, status: "rolled_back", rollback_id: "mock-mcp-rollback" };
  }
  if (pathname.includes("/mcp/rollout/apply")) {
    return { ...base, status: "approved", deployment_id: "mock-mcp-rollout" };
  }
  if (pathname.includes("/agents/releases/rollout/rollback")) {
    return { ...base, status: "rolled_back", rollback_id: "mock-agent-release-rollback" };
  }
  if (pathname.includes("/agents/releases/rollout/apply")) {
    return { ...base, status: "promoted", deployment_id: "mock-agent-release-rollout" };
  }
  if (pathname.includes("/finance/close")) {
    return { ...base, status: "closed", close_id: "mock-finance-close" };
  }
  if (pathname.includes("/finance/reconcile")) {
    return { ...base, status: "reconciled", reconciliation_id: "mock-finance-reconciliation" };
  }
  if (pathname.includes("/vault/kms/rotate")) {
    return { ...base, status: "rotated", rotated_secret_refs: payload.secret_refs || [] };
  }
  if (pathname.includes("/remote-computer/sidecar")) {
    return { ...base, status: "validated", replacement_id: "mock-remote-sidecar-replacement" };
  }

  return { ...base, status: "validated", deployment_id: "mock-validation" };
}

const server = http.createServer(async (request, response) => {
  const url = new URL(request.url, `http://${request.headers.host || `${host}:${port}`}`);
  if (request.method === "GET" && url.pathname === "/healthz") {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ status: "ok" }));
    return;
  }
  if (request.method === "GET" && /^\/v1\/servers\/[^/]+\/tools$/.test(url.pathname)) {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify([{ name: "search", description: "Stage 2 mock MCP search tool" }]));
    return;
  }
  if (request.method === "POST" && url.pathname === "/v1/call") {
    try {
      const payload = await readBody(request);
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({
        result: {
          status: "ok",
          server: payload.server || null,
          tool: payload.tool || null,
          args: payload.args || {},
        },
      }));
    } catch (error) {
      response.writeHead(400, { "content-type": "application/json" });
      response.end(JSON.stringify({ status: "failed", error: error.message }));
    }
    return;
  }
  if (request.method !== "POST") {
    response.writeHead(405, { "content-type": "application/json" });
    response.end(JSON.stringify({ error: "method_not_allowed" }));
    return;
  }
  try {
    const payload = await readBody(request);
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(responseFor(url.pathname, payload)));
  } catch (error) {
    response.writeHead(400, { "content-type": "application/json" });
    response.end(JSON.stringify({ status: "failed", error: error.message }));
  }
});

server.listen(port, host, () => {
  console.error(`stage2 mock controller listening on http://${host}:${port}`);
});
