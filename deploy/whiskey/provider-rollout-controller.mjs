#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.PROVIDER_ROLLOUT_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.PROVIDER_ROLLOUT_CONTROLLER_PORT || "18795");
const controllerToken = process.env.PROVIDER_ROLLOUT_CONTROLLER_TOKEN || "";
const targetEnvironment =
  process.env.PROVIDER_ROLLOUT_CONTROLLER_ENVIRONMENT || "production";

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkAuth(request) {
  if (!controllerToken || request.url === "/healthz") {
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

function validatePayloadType(payload, expectedType) {
  if (payload.type === expectedType) {
    return null;
  }
  return {
    status: "failed",
    message: `unexpected payload type: ${payload.type || "missing"}`,
    steps: [
      step("payload_type", "failed", {
        expected: expectedType,
        observed: payload.type || null,
      }),
    ],
  };
}

async function handleDeploymentValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.provider_deployment");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const providers = Array.isArray(payload.providers) ? payload.providers : [];
  const hasProviders = providers.length > 0;
  const allHealthy = hasProviders && providers.every((provider) => provider.healthy === true);
  const countMatches =
    Number(payload.provider_count || 0) === providers.length &&
    Number(payload.unhealthy_count || 0) === 0;
  const passed = hasProviders && allHealthy && countMatches;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    deployment_id: `whiskey-provider-deployment-${Date.now()}`,
    message: passed
      ? "Whiskey provider deployment validated"
      : "Whiskey provider deployment blocked",
    steps: [
      step("provider_count", hasProviders ? "passed" : "failed", {
        provider_count: providers.length,
      }),
      step("all_providers_healthy", allHealthy ? "passed" : "failed", {
        unhealthy_count: payload.unhealthy_count || 0,
      }),
      step("health_counts_match", countMatches ? "passed" : "failed", {
        reported_provider_count: payload.provider_count || 0,
        reported_unhealthy_count: payload.unhealthy_count || 0,
      }),
    ],
  });
}

async function handleRolloutApply(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.provider_production_rollout");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const providers = Array.isArray(payload.providers) ? payload.providers : [];
  const hasProviders = providers.length > 0;
  const providerIdsMatch =
    Array.isArray(payload.provider_ids) && payload.provider_ids.length === providers.length;
  const enforcementReady =
    payload.production_enforcement?.status === "ready" &&
    payload.production_enforcement?.production_blocked !== true;
  const gatePassed = payload.latest_gate_run_status === "passed";
  const environmentMatches = payload.environment === targetEnvironment;
  const passed = hasProviders && providerIdsMatch && enforcementReady && gatePassed && environmentMatches;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "applied" : "blocked",
    deployment_id: `whiskey-provider-rollout-${Date.now()}`,
    message: passed
      ? "Whiskey provider rollout applied"
      : "Whiskey provider rollout blocked",
    steps: [
      step("provider_selection", hasProviders && providerIdsMatch ? "passed" : "failed", {
        provider_count: providers.length,
        provider_id_count: Array.isArray(payload.provider_ids) ? payload.provider_ids.length : 0,
      }),
      step("policy_enforcement_ready", enforcementReady ? "passed" : "failed", {
        enforcement_status: payload.production_enforcement?.status || "unknown",
      }),
      step("latest_gate_passed", gatePassed ? "passed" : "failed", {
        latest_gate_run_status: payload.latest_gate_run_status || "unknown",
      }),
      step("environment", environmentMatches ? "passed" : "failed", {
        environment: payload.environment || "unknown",
        expected_environment: targetEnvironment,
      }),
    ],
  });
}

async function handleRollback(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.provider_production_rollout_rollback");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const hasProviders = Array.isArray(payload.provider_ids) && payload.provider_ids.length > 0;
  const sourceRolloutApplied = payload.source_rollout?.status === "applied";
  const environmentMatches = payload.environment === targetEnvironment;
  const passed = hasProviders && sourceRolloutApplied && environmentMatches;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "rolled_back" : "blocked",
    rollback_id: `whiskey-provider-rollback-${Date.now()}`,
    message: passed
      ? "Whiskey provider rollout rollback accepted"
      : "Whiskey provider rollout rollback blocked",
    steps: [
      step("provider_ids_present", hasProviders ? "passed" : "failed", {
        provider_id_count: Array.isArray(payload.provider_ids) ? payload.provider_ids.length : 0,
      }),
      step("source_rollout_applied", sourceRolloutApplied ? "passed" : "failed", {
        source_status: payload.source_rollout?.status || "unknown",
      }),
      step("environment", environmentMatches ? "passed" : "failed", {
        environment: payload.environment || "unknown",
        expected_environment: targetEnvironment,
      }),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, {
        status: "ok",
        environment: targetEnvironment,
      });
      return;
    }
    if (!checkAuth(request)) {
      writeJson(response, 401, { status: "unauthorized" });
      return;
    }
    if (request.method === "POST" && request.url === "/provider/deployment/validate") {
      await handleDeploymentValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/provider/rollout/apply") {
      await handleRolloutApply(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/provider/rollout/rollback") {
      await handleRollback(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey provider rollout controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Provider rollout controller listening on http://${listenHost}:${listenPort}`);
});
