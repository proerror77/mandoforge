#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.MCP_PILOT_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.MCP_PILOT_CONTROLLER_PORT || "18792");
const controllerToken = process.env.MCP_PILOT_CONTROLLER_TOKEN || "";
const allowedServer = process.env.MCP_PILOT_SERVER_NAME || "whiskey-docs";
const upstreamMode = process.env.MCP_PILOT_UPSTREAM_MODE || "mock";
const wikimediaApiUrl =
  process.env.MCP_PILOT_WIKIMEDIA_API_URL || "https://en.wikipedia.org/w/api.php";
const wikimediaLimit = Number(process.env.MCP_PILOT_WIKIMEDIA_LIMIT || "5");

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkAuth(request) {
  if (!controllerToken || request.url?.startsWith("/v1/") || request.url === "/healthz") {
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

function step(name, status, details = {}) {
  return { name, ...details, status };
}

async function searchWikimedia(query) {
  const url = new URL(wikimediaApiUrl);
  url.searchParams.set("action", "opensearch");
  url.searchParams.set("search", query);
  url.searchParams.set("limit", String(Math.max(1, wikimediaLimit)));
  url.searchParams.set("namespace", "0");
  url.searchParams.set("format", "json");
  const response = await fetch(url, {
    headers: { "user-agent": "mandoforge-whiskey-mcp-pilot/1.0" },
  });
  if (!response.ok) {
    throw new Error(`wikimedia search failed with status ${response.status}`);
  }
  const body = await response.json();
  const titles = Array.isArray(body?.[1]) ? body[1] : [];
  const descriptions = Array.isArray(body?.[2]) ? body[2] : [];
  const urls = Array.isArray(body?.[3]) ? body[3] : [];
  return titles.map((title, index) => ({
    title,
    url: urls[index] || null,
    snippet: descriptions[index] || "",
    source_id: urls[index] || title,
    reference: title,
    retrieval_actor: "wikimedia-opensearch",
  }));
}

function pilotItems(query) {
  return [
    {
      title: "Whiskey MCP pilot",
      url: "https://example.invalid/mandoforge/whiskey-mcp-pilot",
      snippet: `Pilot response for ${query}`,
      source_id: "whiskey-mcp-pilot",
      reference: "Whiskey MCP pilot",
      retrieval_actor: "whiskey-mcp-pilot",
    },
  ];
}

function validateConnectors(payload) {
  const connectors = Array.isArray(payload.connectors) ? payload.connectors : [];
  const connectorNames = connectors.map((connector) => connector.name).filter(Boolean);
  const unhealthy = connectors.filter((connector) => connector.healthy !== true);
  const hasAllowedServer = connectorNames.includes(allowedServer);
  return {
    connectors,
    unhealthy,
    hasAllowedServer,
    checks: [
      step("connector_count", connectors.length > 0 ? "passed" : "failed", {
        server_count: connectors.length,
      }),
      step("allowed_server_present", hasAllowedServer ? "passed" : "failed", {
        allowed_server: allowedServer,
        connector_names: connectorNames,
      }),
      step("all_connectors_healthy", unhealthy.length === 0 ? "passed" : "failed", {
        unhealthy_count: unhealthy.length,
      }),
    ],
  };
}

async function handleGatewayCall(request, response) {
  const payload = await readJson(request);
  if (payload.server !== allowedServer) {
    writeJson(response, 403, {
      result: {
        status: "blocked",
        message: `server ${payload.server || "unknown"} is not allowed`,
      },
    });
    return;
  }
  if (payload.tool !== "search") {
    writeJson(response, 404, {
      result: {
        status: "not_found",
        message: `tool ${payload.tool || "unknown"} is not available`,
      },
    });
    return;
  }
  const query =
    payload.args?.query || payload.arguments?.query || payload.input?.query || "OpenAI";
  const items =
    upstreamMode === "wikimedia"
      ? await searchWikimedia(String(query))
      : pilotItems(String(query));
  writeJson(response, 200, {
    result: {
      status: "ok",
      source: upstreamMode === "wikimedia" ? "wikimedia-opensearch" : "whiskey-mcp-pilot",
      query,
      item_count: items.length,
      items,
    },
  });
}

async function handleDeploymentValidation(request, response) {
  const payload = await readJson(request);
  if (payload.type !== "mandoforge.mcp_connector_deployment") {
    writeJson(response, 422, {
      status: "failed",
      message: "unexpected deployment payload type",
      steps: [step("payload_type", "failed", { observed: payload.type || null })],
    });
    return;
  }
  const validation = validateConnectors(payload);
  const failed = validation.checks.some((check) => check.status === "failed");
  writeJson(response, failed ? 502 : 200, {
    status: failed ? "failed" : "validated",
    deployment_id: `whiskey-mcp-deployment-${Date.now()}`,
    message: failed
      ? "Whiskey MCP deployment validation found an invalid connector state"
      : "Whiskey MCP connector deployment validated",
    steps: validation.checks,
  });
}

async function handleRolloutApproval(request, response) {
  const payload = await readJson(request);
  if (payload.type !== "mandoforge.mcp_connector_rollout") {
    writeJson(response, 422, {
      status: "failed",
      message: "unexpected rollout payload type",
      steps: [step("payload_type", "failed", { observed: payload.type || null })],
    });
    return;
  }
  const approved = payload.server_name === allowedServer && payload.rollout?.id;
  writeJson(response, approved ? 200 : 502, {
    status: approved ? "approved" : "failed",
    deployment_id: `whiskey-mcp-rollout-${Date.now()}`,
    message: approved
      ? "Whiskey MCP rollout approved"
      : "Whiskey MCP rollout was not approved",
    steps: [
      step("server_allowed", payload.server_name === allowedServer ? "passed" : "failed", {
        server_name: payload.server_name || "unknown",
        allowed_server: allowedServer,
      }),
      step("rollout_id_present", payload.rollout?.id ? "passed" : "failed"),
    ],
  });
}

async function handleRollbackValidation(request, response) {
  const payload = await readJson(request);
  if (payload.type !== "mandoforge.mcp_connector_rollout_rollback") {
    writeJson(response, 422, {
      status: "failed",
      message: "unexpected rollback payload type",
      steps: [step("payload_type", "failed", { observed: payload.type || null })],
    });
    return;
  }
  const rollbackable = payload.server_name === allowedServer && payload.rollout?.id;
  writeJson(response, rollbackable ? 200 : 502, {
    status: rollbackable ? "rolled_back" : "blocked",
    rollback_id: `whiskey-mcp-rollback-${Date.now()}`,
    message: rollbackable
      ? "Whiskey MCP rollback validated"
      : "Whiskey MCP rollback was blocked",
    steps: [
      step("server_allowed", payload.server_name === allowedServer ? "passed" : "failed", {
        server_name: payload.server_name || "unknown",
        allowed_server: allowedServer,
      }),
      step("rollout_id_present", payload.rollout?.id ? "passed" : "failed"),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, { status: "ok" });
      return;
    }
    if (request.method === "GET" && request.url === `/v1/servers/${allowedServer}/tools`) {
      writeJson(response, 200, [
        {
          name: "search",
          description: "Search the Whiskey MCP pilot corpus",
        },
      ]);
      return;
    }
    if (request.method === "POST" && request.url === "/v1/call") {
      await handleGatewayCall(request, response);
      return;
    }
    if (!checkAuth(request)) {
      writeJson(response, 401, { status: "unauthorized" });
      return;
    }
    if (request.method === "POST" && request.url === "/mcp/deployment/validate") {
      await handleDeploymentValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/mcp/rollout/approve") {
      await handleRolloutApproval(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/mcp/rollback/validate") {
      await handleRollbackValidation(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey MCP pilot controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`MCP pilot controller listening on http://${listenHost}:${listenPort}`);
});
