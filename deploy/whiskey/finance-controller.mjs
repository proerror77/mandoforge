#!/usr/bin/env node
import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import http from "node:http";

const execFileAsync = promisify(execFile);

const listenHost = process.env.FINANCE_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.FINANCE_CONTROLLER_PORT || "18798");
const closeToken = process.env.FINANCE_CLOSE_CONTROLLER_TOKEN || "";
const reconciliationToken = process.env.FINANCE_RECONCILIATION_CONTROLLER_TOKEN || "";
const exportDeliveryMode = process.env.FINANCE_EXPORT_DELIVERY_MODE || "accept_only";
const larkCliBin = process.env.FINANCE_EXPORT_LARK_CLI_BIN || "lark-cli";
const larkAs = process.env.FINANCE_EXPORT_LARK_AS || "user";
const larkFolderToken = process.env.FINANCE_EXPORT_LARK_FOLDER_TOKEN || "";
const uploadWorkdir = process.env.FINANCE_EXPORT_UPLOAD_DIR || "/opt/mandoforge-adoption/finance-exports";
const uploadTimeoutMs = Number(process.env.FINANCE_EXPORT_LARK_TIMEOUT_MS || "15000");

const exportState = {
  delivery_count: 0,
  latest_delivery_at: null,
  latest_bytes: 0,
  delivery_mode: exportDeliveryMode,
  latest_file_token: null,
  latest_file_url: null,
  latest_file_name: null,
  latest_error: null,
};
const closeState = {
  close_count: 0,
  latest_close_at: null,
};
const reconciliationState = {
  reconciliation_count: 0,
  latest_reconciliation_at: null,
};

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkBearer(request, token) {
  if (!token || request.url === "/healthz") {
    return true;
  }
  return request.headers.authorization === `Bearer ${token}`;
}

async function readJson(request) {
  const chunks = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > 2 * 1024 * 1024) {
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

async function uploadFinanceExportToLark(filename, csv) {
  if (exportDeliveryMode !== "lark_drive") {
    return {
      delivered: true,
      channel: "accept_only",
      file_token: null,
      file_url: null,
      file_name: filename,
    };
  }

  await fs.mkdir(uploadWorkdir, { recursive: true });
  const safeName = filename.replace(/[^A-Za-z0-9._-]+/g, "-");
  const filePath = path.join(uploadWorkdir, safeName);
  await fs.writeFile(filePath, csv, "utf8");
  try {
    const args = ["drive", "+upload", "--as", larkAs, "--file", `./${safeName}`, "--name", filename];
    if (larkFolderToken) {
      args.push("--folder-token", larkFolderToken);
    }
    const { stdout } = await execFileAsync(larkCliBin, args, {
      cwd: uploadWorkdir,
      timeout: uploadTimeoutMs,
      maxBuffer: 1024 * 1024,
    });
    const response = JSON.parse(stdout || "{}");
    if (response.ok !== true) {
      throw new Error(response.error?.message || "Lark Drive upload was not accepted");
    }
    const data = response.data || {};
    return {
      delivered: true,
      channel: "lark_drive",
      file_token: data.file_token || data.token || null,
      file_url: data.url || data.preview_url || null,
      file_name: data.name || filename,
    };
  } finally {
    await fs.unlink(filePath).catch(() => {});
  }
}

async function handleExportDelivery(request, response) {
  const payload = await readJson(request);
  const validType = payload.type === "mandoforge.usage_finance_export";
  const csvBytes = typeof payload.csv === "string" ? Buffer.byteLength(payload.csv) : 0;
  const filenameMatches = payload.filename === "mandoforge-usage-export.csv";
  const eligible = validType && filenameMatches && csvBytes > 0;
  let uploaded = null;
  if (eligible) {
    try {
      uploaded = await uploadFinanceExportToLark(payload.filename, payload.csv);
    } catch (error) {
      exportState.latest_error = error.message || "finance export delivery failed";
      writeJson(response, 502, {
        status: "failed",
        bytes: csvBytes,
        delivery_mode: exportDeliveryMode,
        target_configured: exportDeliveryMode !== "accept_only",
        message: exportState.latest_error,
      });
      return;
    }
  }
  const passed = eligible && uploaded?.delivered === true;

  if (passed) {
    exportState.delivery_count += 1;
    exportState.latest_delivery_at = new Date().toISOString();
    exportState.latest_bytes = csvBytes;
    exportState.latest_file_token = uploaded.file_token || null;
    exportState.latest_file_url = uploaded.file_url || null;
    exportState.latest_file_name = uploaded.file_name || payload.filename;
    exportState.latest_error = null;
  }

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "delivered" : "blocked",
    delivery_id: `whiskey-finance-export-${Date.now()}`,
    bytes: csvBytes,
    delivery_mode: exportDeliveryMode,
    target_configured: exportDeliveryMode !== "accept_only",
    file_token: uploaded?.file_token || null,
    file_url: uploaded?.file_url || null,
    file_name: uploaded?.file_name || payload.filename || null,
    message: passed
      ? "Whiskey finance export delivered"
      : "Whiskey finance export payload blocked",
    steps: [
      step("payload_type", validType ? "passed" : "failed", {
        observed: payload.type || null,
      }),
      step("filename", filenameMatches ? "passed" : "failed", {
        observed: payload.filename || null,
      }),
      step("csv_nonempty", csvBytes > 0 ? "passed" : "failed", {
        bytes: csvBytes,
      }),
    ],
  });
}

async function handleClose(request, response) {
  if (!checkBearer(request, closeToken)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }
  const payload = await readJson(request);
  const validType = payload.type === "mandoforge.finance_close";
  const exportDelivered = payload.finance_export_delivery_status === "delivered"
    || payload.after?.production_close?.export_recent === true
    || exportState.delivery_count > 0;
  const rollupReady = payload.after?.production_close?.rollup_fresh === true;
  const alertReady = payload.after?.production_close?.alert_delivery_ready !== false
    && payload.after?.production_close?.critical_alerts_acknowledged !== false
    && payload.after?.production_close?.failed_delivery_evidence !== true;
  const passed = validType && exportDelivered && rollupReady && alertReady;

  if (passed) {
    closeState.close_count += 1;
    closeState.latest_close_at = new Date().toISOString();
  }

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "closed" : "blocked",
    close_id: `whiskey-finance-close-${Date.now()}`,
    message: passed
      ? "Whiskey finance close accepted"
      : "Whiskey finance close blocked",
    steps: [
      step("payload_type", validType ? "passed" : "failed", {
        observed: payload.type || null,
      }),
      step("export_delivery", exportDelivered ? "passed" : "failed", {
        finance_export_delivery_status: payload.finance_export_delivery_status || null,
        controller_delivery_count: exportState.delivery_count,
      }),
      step("rollup", rollupReady ? "passed" : "failed", {
        rollup_fresh: payload.after?.production_close?.rollup_fresh ?? null,
      }),
      step("alerts", alertReady ? "passed" : "failed", {
        alert_delivery_ready: payload.after?.production_close?.alert_delivery_ready ?? null,
        critical_alerts_acknowledged:
          payload.after?.production_close?.critical_alerts_acknowledged ?? null,
        failed_delivery_evidence:
          payload.after?.production_close?.failed_delivery_evidence ?? null,
      }),
    ],
  });
}

async function handleReconciliation(request, response) {
  if (!checkBearer(request, reconciliationToken)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }
  const payload = await readJson(request);
  const validType = payload.type === "mandoforge.finance_reconciliation";
  const productionClose = payload.summary?.production_close || {};
  const closeEvidenceReady =
    productionClose.latest_close_controller_closed === true || closeState.close_count > 0;
  const exportRecent = productionClose.export_recent === true || exportState.delivery_count > 0;
  const noFailedDelivery = productionClose.failed_delivery_evidence !== true;
  const passed = validType && closeEvidenceReady && exportRecent && noFailedDelivery;

  if (passed) {
    reconciliationState.reconciliation_count += 1;
    reconciliationState.latest_reconciliation_at = new Date().toISOString();
  }

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "reconciled" : "blocked",
    reconciliation_id: `whiskey-finance-reconciliation-${Date.now()}`,
    message: passed
      ? "Whiskey finance reconciliation accepted"
      : "Whiskey finance reconciliation blocked",
    checks: [
      step("payload_type", validType ? "passed" : "failed", {
        observed: payload.type || null,
      }),
      step("close_evidence", closeEvidenceReady ? "passed" : "failed", {
        latest_close_controller_closed:
          productionClose.latest_close_controller_closed ?? null,
        controller_close_count: closeState.close_count,
      }),
      step("export_recent", exportRecent ? "passed" : "failed", {
        export_recent: productionClose.export_recent ?? null,
        controller_delivery_count: exportState.delivery_count,
      }),
      step("no_failed_delivery", noFailedDelivery ? "passed" : "failed", {
        failed_delivery_evidence: productionClose.failed_delivery_evidence ?? null,
      }),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, {
        status: "ok",
        export_state: exportState,
        close_state: closeState,
        reconciliation_state: reconciliationState,
      });
      return;
    }
    if (request.method === "POST" && request.url === "/finance/export") {
      await handleExportDelivery(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/finance/close") {
      await handleClose(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/finance/reconcile") {
      await handleReconciliation(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey finance controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Finance controller listening on http://${listenHost}:${listenPort}`);
});
