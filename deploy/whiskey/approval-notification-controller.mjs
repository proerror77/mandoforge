#!/usr/bin/env node
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import http from "node:http";

const execFileAsync = promisify(execFile);

const listenHost = process.env.APPROVAL_NOTIFICATION_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.APPROVAL_NOTIFICATION_CONTROLLER_PORT || "18796");
const controllerToken = process.env.APPROVAL_NOTIFICATION_CONTROLLER_TOKEN || "";
const deliveryMode = process.env.APPROVAL_NOTIFICATION_DELIVERY_MODE || "accept_only";
const larkCliBin = process.env.APPROVAL_NOTIFICATION_LARK_CLI_BIN || "lark-cli";
const larkAs = process.env.APPROVAL_NOTIFICATION_LARK_AS || "user";
const larkOpenId = process.env.APPROVAL_NOTIFICATION_LARK_OPEN_ID || "";
const larkTimeoutMs = Number(process.env.APPROVAL_NOTIFICATION_LARK_TIMEOUT_MS || "10000");

const deliveryState = {
  delivered_count: 0,
  latest_delivery_at: null,
  latest_target_count: 0,
  delivery_mode: deliveryMode,
  latest_forwarding_status: "not_attempted",
  latest_forwarding_channel: deliveryMode === "lark_im" ? "lark_im" : "none",
  latest_forwarded_message_id: null,
  latest_forwarded_chat_id: null,
  latest_error: null,
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

function approvalNotificationText(payload) {
  const approvalId = payload.approval?.id || "unknown";
  const status = payload.approval?.status || "pending";
  const reason = payload.approval?.reason || "Whiskey approval notification";
  const targetSubjects = Array.isArray(payload.target_subjects)
    ? payload.target_subjects.join(", ")
    : "unknown";
  return [
    "Whiskey approval notification",
    `approval_id: ${approvalId}`,
    `status: ${status}`,
    `targets: ${targetSubjects}`,
    `reason: ${reason}`,
  ].join("\n");
}

async function forwardApprovalToLark(payload) {
  if (deliveryMode !== "lark_im") {
    return {
      delivered: true,
      channel: "accept_only",
      message_id: null,
      chat_id: null,
    };
  }
  if (!larkOpenId) {
    throw new Error("APPROVAL_NOTIFICATION_LARK_OPEN_ID is required for lark_im delivery");
  }

  const { stdout } = await execFileAsync(
    larkCliBin,
    [
      "im",
      "+messages-send",
      "--as",
      larkAs,
      "--user-id",
      larkOpenId,
      "--text",
      approvalNotificationText(payload),
    ],
    {
      timeout: larkTimeoutMs,
      maxBuffer: 1024 * 1024,
    },
  );
  const response = JSON.parse(stdout || "{}");
  if (response.ok !== true) {
    throw new Error(response.error?.message || "Lark IM delivery was not accepted");
  }
  return {
    delivered: true,
    channel: "lark_im",
    message_id: response.data?.message_id || null,
    chat_id: response.data?.chat_id || null,
  };
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
  const deliveryEligible = payload.type === "mandoforge.approval_requested" && targetCount > 0;
  let forwarded = null;
  try {
    if (deliveryEligible) {
      forwarded = await forwardApprovalToLark(payload);
    }
  } catch (error) {
    deliveryState.latest_forwarding_status = "failed";
    deliveryState.latest_forwarded_message_id = null;
    deliveryState.latest_forwarded_chat_id = null;
    deliveryState.latest_error = error.message || "approval notification forwarding failed";
    writeJson(response, 502, {
      status: "failed",
      delivered: false,
      target_count: targetCount,
      delivery_mode: deliveryMode,
      forwarding_channel: deliveryMode === "lark_im" ? "lark_im" : "none",
      message: deliveryState.latest_error,
    });
    return;
  }
  const delivered = deliveryEligible && forwarded?.delivered === true;
  if (delivered) {
    deliveryState.delivered_count += 1;
    deliveryState.latest_delivery_at = new Date().toISOString();
    deliveryState.latest_target_count = targetCount;
    deliveryState.latest_forwarding_status = "delivered";
    deliveryState.latest_forwarding_channel = forwarded.channel || "unknown";
    deliveryState.latest_forwarded_message_id = forwarded.message_id || null;
    deliveryState.latest_forwarded_chat_id = forwarded.chat_id || null;
    deliveryState.latest_error = null;
  } else if (deliveryEligible) {
    deliveryState.latest_forwarding_status = "blocked";
    deliveryState.latest_error = "delivery did not complete";
  }
  writeJson(response, delivered ? 200 : 422, {
    status: delivered ? "accepted" : "blocked",
    delivered,
    target_count: targetCount,
    delivery_mode: deliveryMode,
    forwarding_channel: forwarded?.channel || "none",
    message_id: forwarded?.message_id || null,
    chat_id: forwarded?.chat_id || null,
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
        latest_forwarding_status: deliveryState.latest_forwarding_status,
        latest_forwarding_channel: deliveryState.latest_forwarding_channel,
        latest_forwarded_message_id: deliveryState.latest_forwarded_message_id,
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
