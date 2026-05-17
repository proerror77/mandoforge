#!/usr/bin/env node
import http from "node:http";

const listenHost = process.env.CODEX_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.CODEX_CONTROLLER_PORT || "18789");
const wsUrl = process.env.CODEX_APP_SERVER_WS_URL || "ws://127.0.0.1:18788";
const controllerToken = process.env.CODEX_CONTROLLER_TOKEN || "";
const timeoutMs = Number(process.env.CODEX_CONTROLLER_TIMEOUT_MS || "5000");

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

async function probeCodexAppServer() {
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        reject(new Error(`Codex App Server probe timed out after ${timeoutMs}ms`));
      }
    }, timeoutMs);
    const socket = new WebSocket(wsUrl);
    const finish = (callback) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      callback();
    };
    socket.onopen = () => {
      socket.send(JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: {
          clientInfo: { name: "mandoforge-whiskey-controller", version: "0.1.0" },
          capabilities: { experimentalApi: true, optOutNotificationMethods: [] },
        },
      }));
    };
    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(String(event.data));
        if (payload.id !== 1) {
          return;
        }
        if (payload.error) {
          finish(() => reject(new Error(`initialize failed: ${JSON.stringify(payload.error)}`)));
          return;
        }
        finish(() => resolve(payload.result || {}));
        socket.close();
      } catch (error) {
        finish(() => reject(error));
      }
    };
    socket.onerror = (error) => {
      finish(() => reject(new Error(error.message || "websocket probe failed")));
    };
    socket.onclose = () => {
      finish(() => reject(new Error("websocket closed before initialize response")));
    };
  });
}

const server = http.createServer(async (request, response) => {
  if (request.method === "GET" && request.url === "/healthz") {
    writeJson(response, 200, { status: "ok" });
    return;
  }
  if (request.method !== "POST" || !["/deployment/validate", "/ops/validate"].includes(request.url)) {
    writeJson(response, 404, { status: "not_found" });
    return;
  }
  if (!checkAuth(request)) {
    writeJson(response, 401, { status: "unauthorized" });
    return;
  }
  try {
    const result = await probeCodexAppServer();
    const kind = request.url.includes("deployment") ? "deployment" : "ops";
    writeJson(response, 200, {
      status: "validated",
      [`${kind}_id`]: `whiskey-codex-${kind}-${Date.now()}`,
      message: "Codex App Server websocket initialize probe passed",
      checks: [
        { name: "websocket_initialize", status: "passed", target: wsUrl },
        { name: "platform", status: "observed", value: result.platformOs || "unknown" },
      ],
    });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Codex App Server websocket initialize probe failed",
      checks: [{ name: "websocket_initialize", status: "failed", target: wsUrl }],
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Codex App Server controller listening on http://${listenHost}:${listenPort}`);
});
