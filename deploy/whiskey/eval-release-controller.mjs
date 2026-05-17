#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.EVAL_RELEASE_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.EVAL_RELEASE_CONTROLLER_PORT || "18793");
const controllerToken = process.env.EVAL_RELEASE_CONTROLLER_TOKEN || "";
const targetEnvironment =
  process.env.EVAL_RELEASE_CONTROLLER_ENVIRONMENT || "whiskey-eval-release";

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

function releaseCounts(payload) {
  return payload.release_counts || {};
}

function latestRun(payload) {
  return payload.automation?.latest_run || null;
}

function latestPromotions(payload) {
  return Array.isArray(payload.latest_promotions) ? payload.latest_promotions : [];
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

async function handleRolloutApply(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.agent_release_rollout");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const hasRelease = Boolean(payload.release_id && payload.agent_id);
  const environmentMatches = payload.environment === targetEnvironment;
  const scoreEligible =
    typeof payload.eval_score === "number" &&
    typeof payload.min_score === "number" &&
    payload.eval_score >= payload.min_score;
  const passed = hasRelease && environmentMatches && scoreEligible;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "promoted" : "blocked",
    deployment_id: `whiskey-agent-release-${Date.now()}`,
    message: passed
      ? "Whiskey agent release rollout promoted"
      : "Whiskey agent release rollout blocked",
    steps: [
      step("release_identity", hasRelease ? "passed" : "failed", {
        release_id: payload.release_id || null,
        agent_id: payload.agent_id || null,
      }),
      step("environment", environmentMatches ? "passed" : "failed", {
        environment: payload.environment || "unknown",
        expected_environment: targetEnvironment,
      }),
      step("eval_gate", scoreEligible ? "passed" : "failed", {
        eval_score: payload.eval_score ?? null,
        min_score: payload.min_score ?? null,
      }),
    ],
  });
}

async function handleDeploymentValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(
    payload,
    "mandoforge.agent_release_deployment_validation",
  );
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const counts = releaseCounts(payload);
  const promotions = latestPromotions(payload);
  const hasPromotedRelease = Number(counts.promoted_count || 0) > 0;
  const hasWhiskeyPromotion = promotions.some(
    (promotion) => promotion.environment === targetEnvironment,
  );
  const productionOpsReady =
    payload.automation?.production_ops?.status === "ready" &&
    payload.automation?.production_ops?.production_blocked !== true;
  const passed = hasPromotedRelease && hasWhiskeyPromotion && productionOpsReady;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    deployment_id: `whiskey-agent-release-deployment-${Date.now()}`,
    message: passed
      ? "Whiskey agent release deployment validated"
      : "Whiskey agent release deployment validation blocked",
    steps: [
      step("promoted_release_present", hasPromotedRelease ? "passed" : "failed", {
        promoted_count: counts.promoted_count || 0,
      }),
      step("target_environment_promoted", hasWhiskeyPromotion ? "passed" : "failed", {
        expected_environment: targetEnvironment,
        latest_promotions: promotions.map((promotion) => promotion.environment),
      }),
      step("production_ops_ready", productionOpsReady ? "passed" : "failed", {
        production_ops_status: payload.automation?.production_ops?.status || "unknown",
      }),
    ],
  });
}

async function handleOrchestrationValidation(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(
    payload,
    "mandoforge.agent_release_orchestration_validation",
  );
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const run = latestRun(payload);
  const counts = releaseCounts(payload);
  const pendingClear =
    Number(counts.pending_count || 0) === 0 &&
    Number(counts.auto_pending_count || 0) === 0 &&
    Number(counts.manual_pending_count || 0) === 0;
  const expiredClear = Number(counts.expired_pending_count || 0) === 0;
  const staleClear = Number(counts.stale_pending_count || 0) === 0;
  const dueRunProcessed = run?.status === "processed" && Number(run.promoted_count || 0) > 0;
  const passed = pendingClear && expiredClear && staleClear && dueRunProcessed;

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    orchestration_id: `whiskey-agent-release-orchestration-${Date.now()}`,
    message: passed
      ? "Whiskey agent release orchestration validated"
      : "Whiskey agent release orchestration validation blocked",
    steps: [
      step("pending_clear", pendingClear ? "passed" : "failed", {
        pending_count: counts.pending_count || 0,
        auto_pending_count: counts.auto_pending_count || 0,
        manual_pending_count: counts.manual_pending_count || 0,
      }),
      step("expired_clear", expiredClear ? "passed" : "failed", {
        expired_pending_count: counts.expired_pending_count || 0,
      }),
      step("stale_clear", staleClear ? "passed" : "failed", {
        stale_pending_count: counts.stale_pending_count || 0,
      }),
      step("due_run_processed", dueRunProcessed ? "passed" : "failed", {
        latest_run_status: run?.status || "none",
        promoted_count: run?.promoted_count || 0,
      }),
    ],
  });
}

async function handleRollback(request, response) {
  const payload = await readJson(request);
  const typeError = validatePayloadType(payload, "mandoforge.agent_release_rollback");
  if (typeError) {
    writeJson(response, 422, typeError);
    return;
  }

  const release = payload.release || {};
  const rollbackable =
    release.status === "promoted" &&
    release.release_id &&
    release.agent_id &&
    release.environment === targetEnvironment;

  writeJson(response, rollbackable ? 200 : 502, {
    status: rollbackable ? "rolled_back" : "blocked",
    rollback_id: `whiskey-agent-release-rollback-${Date.now()}`,
    message: rollbackable
      ? "Whiskey agent release rollback validated"
      : "Whiskey agent release rollback blocked",
    steps: [
      step("release_promoted", release.status === "promoted" ? "passed" : "failed", {
        release_status: release.status || "unknown",
      }),
      step("release_identity", release.release_id && release.agent_id ? "passed" : "failed", {
        release_id: release.release_id || null,
        agent_id: release.agent_id || null,
      }),
      step("environment", release.environment === targetEnvironment ? "passed" : "failed", {
        environment: release.environment || "unknown",
        expected_environment: targetEnvironment,
      }),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, { status: "ok" });
      return;
    }
    if (!checkAuth(request)) {
      writeJson(response, 401, { status: "unauthorized" });
      return;
    }
    if (request.method === "POST" && request.url === "/agents/releases/rollout/apply") {
      await handleRolloutApply(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/agents/releases/deployment/validate") {
      await handleDeploymentValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/agents/releases/orchestration/validate") {
      await handleOrchestrationValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/agents/releases/rollout/rollback") {
      await handleRollback(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey eval/release controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Eval/release controller listening on http://${listenHost}:${listenPort}`);
});
