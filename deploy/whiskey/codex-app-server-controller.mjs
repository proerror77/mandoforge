#!/usr/bin/env node
import http from "node:http";
import { randomUUID } from "node:crypto";

const listenHost = process.env.CODEX_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.CODEX_CONTROLLER_PORT || "18789");
const wsUrl = process.env.CODEX_APP_SERVER_WS_URL || "ws://127.0.0.1:18788";
const controllerToken = process.env.CODEX_CONTROLLER_TOKEN || "";
const timeoutMs = Number(process.env.CODEX_CONTROLLER_TIMEOUT_MS || "30000");
const runs = new Map();
const threads = new Map();

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function readBody(request) {
  return new Promise((resolve, reject) => {
    let data = "";
    request.on("data", (chunk) => {
      data += chunk;
      if (data.length > 1024 * 1024) {
        request.destroy(new Error("request body too large"));
      }
    });
    request.on("end", () => {
      if (!data.trim()) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(data));
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });
}

function checkAuth(request) {
  if (!controllerToken) {
    return true;
  }
  return request.headers.authorization === `Bearer ${controllerToken}`;
}

function normalizeError(error) {
  return error?.message || String(error) || "Codex App Server request failed";
}

function socketSend(socket, payload) {
  socket.send(JSON.stringify(payload));
}

async function openCodexSocket() {
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        reject(new Error(`Codex App Server initialize timed out after ${timeoutMs}ms`));
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
      socketSend(socket, {
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: {
          clientInfo: { name: "mandoforge-whiskey-controller", version: "0.1.0" },
          capabilities: { experimentalApi: true, optOutNotificationMethods: [] },
        },
      });
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
        socketSend(socket, { jsonrpc: "2.0", method: "initialized" });
        finish(() => resolve({ socket, initializeResult: payload.result || {} }));
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

async function callCodex(method, params) {
  const { socket, initializeResult } = await openCodexSocket();
  const id = 2;
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        socket.close();
        reject(new Error(`${method} timed out after ${timeoutMs}ms`));
      }
    }, timeoutMs);
    const finish = (callback) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      callback();
    };
    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(String(event.data));
        if (payload.id !== id) {
          return;
        }
        if (payload.error) {
          finish(() => reject(new Error(`${method} failed: ${JSON.stringify(payload.error)}`)));
          socket.close();
          return;
        }
        finish(() => resolve({ result: payload.result || {}, initializeResult }));
        socket.close();
      } catch (error) {
        finish(() => reject(error));
        socket.close();
      }
    };
    socket.onclose = () => {
      finish(() => reject(new Error(`websocket closed before ${method} response`)));
    };
    socketSend(socket, { jsonrpc: "2.0", id, method, params });
  });
}

async function createThread(body) {
  const { socket, initializeResult } = await openCodexSocket();
  const id = 2;
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        socket.close();
        reject(new Error(`thread/start timed out after ${timeoutMs}ms`));
      }
    }, timeoutMs);
    const finish = (callback) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      callback();
    };
    const notifications = [];
    socket.onmessage = (event) => {
      try {
        const payload = JSON.parse(String(event.data));
        if (payload.id === id) {
          if (payload.error) {
            finish(() => reject(new Error(`thread/start failed: ${JSON.stringify(payload.error)}`)));
            socket.close();
            return;
          }
          const result = payload.result || {};
          const response = toThreadResponse(result);
          threads.set(response.thread_id, {
            thread_id: response.thread_id,
            socket,
            initializeResult,
            metadata: result,
            notifications,
          });
          finish(() => resolve(response));
          return;
        }
        if (payload.method) {
          notifications.push(payload);
        }
      } catch (error) {
        finish(() => reject(error));
        socket.close();
      }
    };
    socket.onclose = () => {
      finish(() => reject(new Error("websocket closed before thread/start response")));
    };
    socketSend(socket, {
      jsonrpc: "2.0",
      id,
      method: "thread/start",
      params: {
        cwd: body.metadata?.workspace || body.cwd || null,
        approvalPolicy: body.approvalPolicy || "never",
        sandbox: body.sandbox || "workspace-write",
        model: body.model || null,
        modelProvider: body.modelProvider || null,
        serviceName: "mandoforge",
        ephemeral: true,
        sessionStartSource: "startup",
        threadSource: "user",
      },
    });
  });
}

async function startTurn(threadId, body) {
  const thread = threads.get(threadId);
  if (!thread?.socket) {
    throw new Error(`unknown or disconnected Codex thread ${threadId}`);
  }
  const socket = thread.socket;
  const id = 3 + runs.size;
  const turnIdFallback = `turn-${randomUUID()}`;
  const run = {
    thread_id: threadId,
    turn_id: turnIdFallback,
    status: "inProgress",
    result: { notifications: [] },
    socket,
    started_at: new Date().toISOString(),
  };

  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        socket.close();
        reject(new Error(`turn/start timed out after ${timeoutMs}ms`));
      }
    }, timeoutMs);
    const finishStart = (callback) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      callback();
    };
    socket.onmessage = (event) => {
      let payload;
      try {
        payload = JSON.parse(String(event.data));
      } catch (error) {
        run.result.notifications.push({ method: "parse_error", error: normalizeError(error) });
        return;
      }
      if (payload.id === id) {
        if (payload.error) {
          finishStart(() => reject(new Error(`turn/start failed: ${JSON.stringify(payload.error)}`)));
          socket.close();
          return;
        }
        const turn = payload.result?.turn || {};
        run.turn_id = turn.id || turn.turnId || run.turn_id;
        run.status = statusText(turn.status, "inProgress");
        run.result.turn_start = payload.result || {};
        runs.set(run.turn_id, run);
        finishStart(() => resolve(toTurnResponse(run)));
        return;
      }
      if (payload.method) {
        run.result.notifications.push(payload);
        if (payload.method === "thread/status/changed") {
          const threadStatus = statusText(payload.params?.status, "");
          if (threadStatus === "systemError") {
            run.status = "failed";
            run.result.error = "Codex App Server thread entered systemError";
            run.completed_at = new Date().toISOString();
            try {
              socket.close();
            } catch {
              // Nothing to do: the run is already terminal.
            }
          }
        }
        if (payload.method === "turn/completed") {
          const turn = payload.params?.turn || {};
          run.status = statusText(turn.status, "completed");
          run.result.turn_completed = payload.params || {};
          run.completed_at = new Date().toISOString();
          try {
            socket.close();
          } catch {
            // Nothing to do: the run is already terminal.
          }
        }
      }
    };
    socket.onerror = (error) => {
      if (run.status === "inProgress") {
        run.status = "failed";
        run.result.error = normalizeError(error);
        run.completed_at = new Date().toISOString();
      }
      finishStart(() => reject(new Error(run.result.error)));
    };
    socket.onclose = () => {
      if (run.status === "inProgress" && run.result.turn_start) {
        run.status = "failed";
        run.result.error = "websocket closed before turn completed";
        run.completed_at = new Date().toISOString();
      }
      threads.delete(threadId);
      finishStart(() => reject(new Error("websocket closed before turn/start response")));
    };
    const input = Array.isArray(body.input)
      ? body.input
      : [{ type: "text", text: body.message || "", text_elements: [] }];
    socketSend(socket, {
      jsonrpc: "2.0",
      id,
      method: "turn/start",
      params: {
        threadId,
        input,
        cwd: body.metadata?.workspace || body.cwd || null,
        approvalPolicy: body.approvalPolicy || "never",
        sandboxPolicy: body.sandboxPolicy || null,
        model: body.model || null,
      },
    });
  });
}

function toThreadResponse(result) {
  const thread = result.thread || {};
  return {
    thread_id: thread.id,
    status: statusText(thread.status, "ready"),
    metadata: result,
  };
}

function toTurnResponse(run) {
  return {
    turn_id: run.turn_id,
    thread_id: run.thread_id,
    status: run.status,
    result: run.result,
  };
}

function statusText(status, fallback) {
  if (typeof status === "string") {
    return status;
  }
  if (status && typeof status.type === "string") {
    return status.type;
  }
  return fallback;
}

async function probeCodexAppServer() {
  const { socket, initializeResult } = await openCodexSocket();
  socket.close();
  return initializeResult;
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, { status: "ok", upstream: wsUrl });
      return;
    }
    if (request.method === "POST" && ["/deployment/validate", "/ops/validate"].includes(request.url)) {
      if (!checkAuth(request)) {
        writeJson(response, 401, { status: "unauthorized" });
        return;
      }
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
      return;
    }
    if (request.method === "POST" && request.url === "/threads") {
      const body = await readBody(request);
      writeJson(response, 200, await createThread(body));
      return;
    }
    const turnCreateMatch = request.url.match(/^\/threads\/([^/]+)\/turns$/);
    if (request.method === "POST" && turnCreateMatch) {
      const body = await readBody(request);
      const turn = await startTurn(decodeURIComponent(turnCreateMatch[1]), body);
      writeJson(response, 200, turn);
      return;
    }
    const turnMatch = request.url.match(/^\/turns\/([^/]+)$/);
    if (request.method === "GET" && turnMatch) {
      const turnId = decodeURIComponent(turnMatch[1]);
      const run = runs.get(turnId);
      if (!run) {
        writeJson(response, 404, { status: "not_found", message: `unknown turn ${turnId}` });
        return;
      }
      writeJson(response, 200, toTurnResponse(run));
      return;
    }
    const interruptMatch = request.url.match(/^\/turns\/([^/]+)\/interrupt$/);
    if (request.method === "POST" && interruptMatch) {
      const turnId = decodeURIComponent(interruptMatch[1]);
      const run = runs.get(turnId);
      if (!run) {
        writeJson(response, 404, { status: "not_found", message: `unknown turn ${turnId}` });
        return;
      }
      if (run.socket) {
        socketSend(run.socket, {
          jsonrpc: "2.0",
          id: 99,
          method: "turn/interrupt",
          params: { threadId: run.thread_id, turnId: run.turn_id },
        });
      }
      run.status = "interrupted";
      run.completed_at = new Date().toISOString();
      writeJson(response, 200, { turn_id: run.turn_id, status: run.status });
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: normalizeError(error),
      upstream: wsUrl,
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`Codex App Server controller listening on http://${listenHost}:${listenPort}`);
});
