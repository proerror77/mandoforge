#!/usr/bin/env node
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import http from "node:http";

const execFileAsync = promisify(execFile);

const listenHost = process.env.WORKER_LOAD_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.WORKER_LOAD_CONTROLLER_PORT || "18791");
const controllerToken = process.env.WORKER_LOAD_CONTROLLER_TOKEN || "";
const apiBaseUrl = (process.env.WORKER_LOAD_CONTROLLER_API_URL || "http://127.0.0.1:18787").replace(/\/$/, "");
const apiToken = process.env.WORKER_LOAD_CONTROLLER_API_TOKEN || "";
const apiSubject = process.env.WORKER_LOAD_CONTROLLER_API_SUBJECT || "whiskey-worker-load-controller";
const apiRoles = process.env.WORKER_LOAD_CONTROLLER_API_ROLES || "admin";
const composeFile = process.env.WORKER_LOAD_CONTROLLER_COMPOSE_FILE || "/opt/mandoforge-adoption/docker-compose.yml";
const composeProject = process.env.WORKER_LOAD_CONTROLLER_COMPOSE_PROJECT || "mandoforge-adoption";
const timeoutMs = Number(process.env.WORKER_LOAD_CONTROLLER_TIMEOUT_MS || "5000");

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
  const headers = apiToken
    ? { authorization: `Bearer ${apiToken}` }
    : {
        "x-mandoforge-subject": apiSubject,
        "x-mandoforge-roles": apiRoles,
      };
  try {
    const response = await fetch(`${apiBaseUrl}${path}`, {
      signal: controller.signal,
      headers,
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

async function workerServiceRunning() {
  const { stdout } = await execFileAsync("docker", [
    "compose",
    "-p",
    composeProject,
    "-f",
    composeFile,
    "ps",
    "--status",
    "running",
    "--services",
  ], { timeout: timeoutMs });
  return stdout.split(/\s+/).includes("worker");
}

function check(name, status, details = {}) {
  return { name, ...details, status };
}

function buildChecks(payload, readiness, workerRunning) {
  const jobSummary = readiness.job_summary || {};
  const leaseSummary = readiness.lease_summary || {};
  const queueBackend = readiness.queue_backend || {};
  const workerMode = readiness.worker_mode || {};
  const k8s = readiness.k8s || {};
  const autoscaling = readiness.autoscaling || {};

  return [
    check("api_health", "passed", { target: `${apiBaseUrl}/healthz` }),
    check("worker_container_running", workerRunning ? "passed" : "failed", {
      compose_project: composeProject,
    }),
    check("payload_queue_matches_live_api", payload.queue_backend?.kind === queueBackend.kind ? "passed" : "failed", {
      payload_queue_backend: payload.queue_backend?.kind || "unknown",
      live_queue_backend: queueBackend.kind || "unknown",
    }),
    check("durable_queue", queueBackend.durable ? "passed" : "failed", {
      queue_backend: queueBackend.kind || "unknown",
    }),
    check("queue_worker_mode", workerMode.mode === "queue" ? "passed" : "failed", {
      worker_mode: workerMode.mode || "unknown",
    }),
    check("worker_pod_hardening_manifest", k8s.hardening_status === "hardened" ? "passed" : "failed", {
      hardening_status: k8s.hardening_status || "unknown",
    }),
    check("queue_depth_autoscaling_manifest", autoscaling.validation_status === "queue_depth_configured" ? "passed" : "failed", {
      autoscaling_status: autoscaling.validation_status || "unknown",
    }),
    check("isolated_worker_pool_manifest", payload.isolated_worker_pool_manifest_configured ? "passed" : "failed"),
    check("no_failed_jobs", Number(jobSummary.failed_jobs || 0) === 0 ? "passed" : "failed", {
      failed_jobs: Number(jobSummary.failed_jobs || 0),
    }),
    check("no_stale_worker_leases", Number(leaseSummary.stale_leases || 0) === 0 ? "passed" : "failed", {
      stale_leases: Number(leaseSummary.stale_leases || 0),
    }),
    check("whiskey_single_host_scope", "passed", {
      scope: "single-host worker load evidence; not a k3s or multi-replica autoscaling proof",
    }),
  ];
}

const server = http.createServer(async (request, response) => {
  if (request.method === "GET" && request.url === "/healthz") {
    writeJson(response, 200, { status: "ok" });
    return;
  }
  if (request.method !== "POST" || request.url !== "/worker/load/validate") {
    writeJson(response, 404, { status: "not_found" });
    return;
  }
  if (!checkAuth(request)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }

  try {
    const payload = await readJson(request);
    if (payload.type !== "mandoforge.worker_load_validation") {
      writeJson(response, 422, {
        status: "failed",
        message: "unexpected validation payload type",
        load_validated: false,
        isolated_worker_pool_configured: false,
        checks: [check("payload_type", "failed", { observed: payload.type || null })],
      });
      return;
    }

    await apiFetch("/healthz");
    const readiness = await apiFetch("/api/execution-jobs/worker-readiness");
    const workerRunning = await workerServiceRunning();
    const checks = buildChecks(payload, readiness, workerRunning);
    const hardFailed = checks.some((item) => item.status === "failed");

    writeJson(response, hardFailed ? 502 : 200, {
      status: hardFailed ? "failed" : "validated",
      validation_id: `whiskey-worker-load-${Date.now()}`,
      message: hardFailed
        ? "Whiskey worker load controller found an unsafe single-host worker signal"
        : "Whiskey single-host worker load evidence validated",
      load_validated: !hardFailed,
      isolated_worker_pool_configured: Boolean(payload.isolated_worker_pool_manifest_configured),
      observed_replicas: {
        min: 1,
        max: 1,
        scaled_from: 1,
        scaled_to: 1,
        scope: "whiskey-single-host",
      },
      checks,
    });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey worker load validation failed",
      load_validated: false,
      isolated_worker_pool_configured: false,
      checks: [check("controller_execution", "failed", { target: apiBaseUrl })],
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Worker load controller listening on http://${listenHost}:${listenPort}`);
});
