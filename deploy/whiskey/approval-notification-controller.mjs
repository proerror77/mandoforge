#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.APPROVAL_NOTIFICATION_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.APPROVAL_NOTIFICATION_CONTROLLER_PORT || "18796");
const controllerToken = process.env.APPROVAL_NOTIFICATION_CONTROLLER_TOKEN || "";

const deliveryState = {
  delivered_count: 0,
  latest_delivery_at: null,
  latest_target_count: 0,
};

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkAuth(request) {
  if (!controllerToken || request.url === "/healthz" || request.url === "/approval/webhook") {
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

async function handleWebhookDelivery(request, response) {
  const payload = await readJson(request);
  const targetCount = Number(payload.target_count || 0);
  const delivered = payload.type === "mandoforge.approval_requested" && targetCount > 0;
  if (delivered) {
    deliveryState.delivered_count += 1;
    deliveryState.latest_delivery_at = new Date().toISOString();
    deliveryState.latest_target_count = targetCount;
  }
  writeJson(response, delivered ? 200 : 422, {
    status: delivered ? "accepted" : "blocked",
    delivered,
    target_count: targetCount,
  });
}

async function handleDeploymentValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.approval_notification_deployment");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const routing = payload.routing || {};
  const hasChannel = Number(routing.channel_count || 0) > 0 && routing.webhook_configured === true;
  const hasPolicy = Number(routing.active_policy_count || 0) > 0;
  const routable = Number(routing.unroutable_pending_count || 0) === 0;
  const passed = hasChannel && hasPolicy && routable;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    deployment_id: `whiskey-approval-notification-deployment-${Date.now()}`,
    message: passed
      ? "Whiskey approval notification deployment validated"
      : "Whiskey approval notification deployment blocked",
    steps: [
      step("webhook_channel_configured", hasChannel ? "passed" : "failed", {
        channel_count: routing.channel_count || 0,
        webhook_configured: routing.webhook_configured === true,
      }),
      step("active_policy_present", hasPolicy ? "passed" : "failed", {
        active_policy_count: routing.active_policy_count || 0,
      }),
      step("pending_approvals_routable", routable ? "passed" : "failed", {
        unroutable_pending_count: routing.unroutable_pending_count || 0,
      }),
    ],
  });
}

async function handleOpsValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.approval_notification_ops");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const routing = payload.routing || {};
  const delivery = payload.delivery || {};
  const routingReady =
    Number(routing.channel_count || 0) > 0 &&
    Number(routing.active_policy_count || 0) > 0 &&
    Number(routing.unroutable_pending_count || 0) === 0;
  const controllerReachable = true;

  writeJson(response, routingReady ? 200 : 502, {
    status: routingReady ? "validated" : "blocked",
    ops_id: `whiskey-approval-notification-ops-${Date.now()}`,
    message: routingReady
      ? "Whiskey approval notification ops validated"
      : "Whiskey approval notification ops blocked",
    checks: [
      step("routing_ready", routingReady ? "passed" : "failed", {
        routing_status: routing.status || "unknown",
        channel_count: routing.channel_count || 0,
        active_policy_count: routing.active_policy_count || 0,
        unroutable_pending_count: routing.unroutable_pending_count || 0,
      }),
      step("delivery_history_observed", "passed", {
        latest_run_status: delivery.latest_run_status || "none",
        delivered_count: deliveryState.delivered_count,
      }),
      step("controller_reachable", controllerReachable ? "passed" : "failed"),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, {
        status: "ok",
        delivery: deliveryState,
      });
      return;
    }
    if (request.method === "POST" && request.url === "/approval/webhook") {
      await handleWebhookDelivery(request, response);
      return;
    }
    if (!checkAuth(request)) {
      writeJson(response, 401, { status: "unauthorized" });
      return;
    }
    if (request.method === "POST" && request.url === "/approval-notification/deployment/validate") {
      await handleDeploymentValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/approval-notification/ops/validate") {
      await handleOpsValidation(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey approval notification controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Approval notification controller listening on http://${listenHost}:${listenPort}`);
});
