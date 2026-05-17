#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.VAULT_KMS_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.VAULT_KMS_CONTROLLER_PORT || "18797");
const controllerToken = process.env.VAULT_KMS_CONTROLLER_TOKEN || "";
const vaultToken = process.env.VAULT_KMS_CONTROLLER_VAULT_TOKEN || "";
const kmsProvider = process.env.VAULT_KMS_CONTROLLER_PROVIDER || "mock-kms";
const kmsKeyId = process.env.VAULT_KMS_CONTROLLER_KEY_ID || "whiskey-kms-key-1";
const rotationPolicy =
  process.env.VAULT_KMS_CONTROLLER_ROTATION_POLICY || "whiskey-manual-confirmed";

const secretStore = new Map();
const rotationState = {
  rotation_count: 0,
  latest_rotation_at: null,
  latest_rotated_count: 0,
};
const recoveryState = {
  recovery_count: 0,
  latest_recovery_at: null,
};

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkBearer(request) {
  if (!controllerToken || request.url === "/healthz") {
    return true;
  }
  return request.headers.authorization === `Bearer ${controllerToken}`;
}

function checkVaultToken(request) {
  if (!vaultToken) {
    return true;
  }
  return request.headers["x-vault-token"] === vaultToken;
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

function vaultPathFromUrl(url) {
  const prefix = "/v1/kv/data/";
  if (!url?.startsWith(prefix)) {
    return "";
  }
  return decodeURIComponent(url.slice(prefix.length));
}

async function handleVaultKvWrite(request, response) {
  if (!checkVaultToken(request)) {
    writeJson(response, 403, { errors: ["invalid vault token"] });
    return;
  }
  const path = vaultPathFromUrl(request.url);
  if (!path) {
    writeJson(response, 404, { errors: ["not found"] });
    return;
  }
  const payload = await readJson(request);
  secretStore.set(path, payload.data || {});
  writeJson(response, 200, {
    data: {
      created_time: new Date().toISOString(),
      version: 1,
    },
  });
}

async function handleVaultKvRead(request, response) {
  if (!checkVaultToken(request)) {
    writeJson(response, 403, { errors: ["invalid vault token"] });
    return;
  }
  const path = vaultPathFromUrl(request.url);
  if (!path || !secretStore.has(path)) {
    writeJson(response, 404, { errors: ["secret not found"] });
    return;
  }
  writeJson(response, 200, {
    data: {
      data: secretStore.get(path),
      metadata: {
        version: 1,
      },
    },
  });
}

async function handleKmsRotation(request, response) {
  if (!checkBearer(request)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }
  const payload = await readJson(request);
  const secretRecords = Array.isArray(payload.secret_records) ? payload.secret_records : [];
  const providerMatches = payload.provider === kmsProvider;
  const keyMatches = payload.key_id === kmsKeyId;
  const policyMatches = payload.rotation_policy === rotationPolicy;
  const validType = payload.type === "mandoforge.kms_rotation_validation";
  const passed = validType && providerMatches && keyMatches && policyMatches;
  const rotatedIds = passed
    ? secretRecords
        .map((record) => record.id)
        .filter((id) => typeof id === "string" && id.length > 0)
    : [];

  if (passed) {
    rotationState.rotation_count += 1;
    rotationState.latest_rotation_at = new Date().toISOString();
    rotationState.latest_rotated_count = rotatedIds.length;
  }

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "rotated" : "blocked",
    rotation_id: `whiskey-kms-rotation-${Date.now()}`,
    rotated_count: rotatedIds.length,
    rotated_secret_record_ids: rotatedIds,
    actions: passed ? ["whiskey_kms_rotation_confirmed"] : ["fix_whiskey_kms_payload"],
    message: passed
      ? "Whiskey KMS rotation validated"
      : "Whiskey KMS rotation payload was blocked",
    steps: [
      step("payload_type", validType ? "passed" : "failed", {
        observed: payload.type || null,
      }),
      step("provider", providerMatches ? "passed" : "failed", {
        observed: payload.provider || null,
        expected: kmsProvider,
      }),
      step("key_id", keyMatches ? "passed" : "failed", {
        observed: payload.key_id || null,
        expected: kmsKeyId,
      }),
      step("rotation_policy", policyMatches ? "passed" : "failed", {
        observed: payload.rotation_policy || null,
        expected: rotationPolicy,
      }),
      step("secret_records", "passed", {
        secret_record_count: secretRecords.length,
      }),
    ],
  });
}

async function handleKmsRecovery(request, response) {
  if (!checkBearer(request)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }
  const payload = await readJson(request);
  const validType = payload.type === "mandoforge.kms_recovery_validation";
  const providerMatches = payload.kms?.provider === kmsProvider;
  const kmsReady = payload.kms?.status === "ready";
  const latestRotationValidated = payload.readiness?.latest_rotation_validated === true;
  const passed = validType && providerMatches && kmsReady && latestRotationValidated;

  if (passed) {
    recoveryState.recovery_count += 1;
    recoveryState.latest_recovery_at = new Date().toISOString();
  }

  writeJson(response, passed ? 200 : 502, {
    status: passed ? "validated" : "blocked",
    recovery_id: `whiskey-kms-recovery-${Date.now()}`,
    message: passed
      ? "Whiskey KMS recovery drill validated"
      : "Whiskey KMS recovery drill blocked",
    steps: [
      step("payload_type", validType ? "passed" : "failed", {
        observed: payload.type || null,
      }),
      step("provider", providerMatches ? "passed" : "failed", {
        observed: payload.kms?.provider || null,
        expected: kmsProvider,
      }),
      step("kms_ready", kmsReady ? "passed" : "failed", {
        kms_status: payload.kms?.status || "unknown",
      }),
      step("recent_rotation", latestRotationValidated ? "passed" : "failed"),
      step("secret_metadata_only", "passed", {
        secret_ref_count: Array.isArray(payload.secret_refs) ? payload.secret_refs.length : 0,
      }),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, {
        status: "ok",
        kms_provider: kmsProvider,
        kms_key_id: kmsKeyId,
        rotation_policy: rotationPolicy,
        rotation_state: rotationState,
        recovery_state: recoveryState,
      });
      return;
    }
    if (request.method === "GET" && request.url === "/v1/sys/health") {
      if (!checkVaultToken(request)) {
        writeJson(response, 403, { errors: ["invalid vault token"] });
        return;
      }
      writeJson(response, 200, {
        initialized: true,
        sealed: false,
        standby: false,
        version: "whiskey-pilot",
      });
      return;
    }
    if (request.method === "POST" && request.url?.startsWith("/v1/kv/data/")) {
      await handleVaultKvWrite(request, response);
      return;
    }
    if (request.method === "GET" && request.url?.startsWith("/v1/kv/data/")) {
      await handleVaultKvRead(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/kms/rotate") {
      await handleKmsRotation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/kms/recovery/validate") {
      await handleKmsRecovery(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey Vault/KMS controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Vault/KMS controller listening on http://${listenHost}:${listenPort}`);
});
