#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.OBSERVABILITY_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.OBSERVABILITY_CONTROLLER_PORT || "18794");
const controllerToken = process.env.OBSERVABILITY_CONTROLLER_TOKEN || "";
const serviceName = process.env.OBSERVABILITY_CONTROLLER_SERVICE_NAME || "mandoforge-api";

const receivedSignals = {
  logs: 0,
  traces: 0,
  metrics: 0,
};

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkAuth(request) {
  if (
    !controllerToken ||
    request.url === "/healthz" ||
    request.url?.startsWith("/v1/")
  ) {
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

async function handleOtlpSignal(request, response, signal) {
  await readJson(request);
  receivedSignals[signal] += 1;
  writeJson(response, 200, {
    status: "ok",
    signal,
    received_count: receivedSignals[signal],
  });
}

async function handleDeploymentValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.observability_collector_deployment");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const endpointConfigured = Boolean(payload.otlp_endpoint);
  const signalPaths = payload.signal_paths || {};
  const signalPathsConfigured = ["logs", "traces", "metrics"].every((signal) =>
    Boolean(signalPaths[signal]),
  );
  const serviceMatches = payload.service_name === serviceName;
  const passed = endpointConfigured && signalPathsConfigured && serviceMatches;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    deployment_id: `whiskey-otel-deployment-${Date.now()}`,
    message: passed
      ? "Whiskey OTel collector deployment validated"
      : "Whiskey OTel collector deployment blocked",
    steps: [
      step("otlp_endpoint_configured", endpointConfigured ? "passed" : "failed", {
        otlp_endpoint: payload.otlp_endpoint || null,
      }),
      step("signal_paths_configured", signalPathsConfigured ? "passed" : "failed", {
        signal_paths: signalPaths,
      }),
      step("service_name", serviceMatches ? "passed" : "failed", {
        service_name: payload.service_name || "unknown",
        expected_service_name: serviceName,
      }),
    ],
  });
}

async function handleClusterValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(
    payload,
    "mandoforge.observability_collector_cluster_rollout",
  );
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const deploymentValidated =
    payload.deployment_readiness?.deployment_validated === true &&
    payload.deployment_readiness?.latest_controller_status === "validated";
  const otlpEnabled = payload.otlp_enabled === true;
  const endpointConfigured = payload.otlp_endpoint_configured === true;
  const passed = deploymentValidated && otlpEnabled && endpointConfigured;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    cluster_rollout_id: `whiskey-otel-cluster-${Date.now()}`,
    message: passed
      ? "Whiskey OTel collector cluster rollout validated"
      : "Whiskey OTel collector cluster rollout blocked",
    steps: [
      step("deployment_validated", deploymentValidated ? "passed" : "failed", {
        deployment_readiness: payload.deployment_readiness || {},
      }),
      step("otlp_enabled", otlpEnabled ? "passed" : "failed"),
      step("endpoint_configured", endpointConfigured ? "passed" : "failed"),
    ],
  });
}

async function handleRemediation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.observability_remediation");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const actions = Array.isArray(payload.actions) ? payload.actions : [];
  const hasActions = actions.length > 0;
  writeJson(response, hasActions ? 200 : 502, {
    status: hasActions ? "remediated" : "blocked",
    remediation_id: `whiskey-otel-remediation-${Date.now()}`,
    message: hasActions
      ? "Whiskey observability remediation controller accepted actions"
      : "Whiskey observability remediation controller found no actions",
    steps: [
      step("actions_present", hasActions ? "passed" : "failed", {
        actions,
      }),
      step("before_after_present", payload.before && payload.after ? "passed" : "failed", {
        before_status: payload.before?.status || "unknown",
        after_status: payload.after?.status || "unknown",
      }),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, {
        status: "ok",
        service_name: serviceName,
        received_signals: receivedSignals,
      });
      return;
    }
    if (request.method === "POST" && request.url === "/v1/logs") {
      await handleOtlpSignal(request, response, "logs");
      return;
    }
    if (request.method === "POST" && request.url === "/v1/traces") {
      await handleOtlpSignal(request, response, "traces");
      return;
    }
    if (request.method === "POST" && request.url === "/v1/metrics") {
      await handleOtlpSignal(request, response, "metrics");
      return;
    }
    if (!checkAuth(request)) {
      writeJson(response, 401, { status: "unauthorized" });
      return;
    }
    if (request.method === "POST" && request.url === "/observability/collector/deployment/validate") {
      await handleDeploymentValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/observability/collector/cluster/validate") {
      await handleClusterValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/observability/remediation/run") {
      await handleRemediation(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey observability controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Observability controller listening on http://${listenHost}:${listenPort}`);
});
