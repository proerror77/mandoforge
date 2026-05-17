#!/usr/bin/env node
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import http from "node:http";

const execFileAsync = promisify(execFile);

const listenHost = process.env.MCP_PILOT_CONTROLLER_HOST || "127.0.0.1";
const listenPort = Number(process.env.MCP_PILOT_CONTROLLER_PORT || "18792");
const controllerToken = process.env.MCP_PILOT_CONTROLLER_TOKEN || "";
const allowedServer = process.env.MCP_PILOT_SERVER_NAME || "whiskey-docs";
const upstreamMode = process.env.MCP_PILOT_UPSTREAM_MODE || "mock";
const wikimediaApiUrl =
  process.env.MCP_PILOT_WIKIMEDIA_API_URL || "https://en.wikipedia.org/w/api.php";
const wikimediaLimit = Number(process.env.MCP_PILOT_WIKIMEDIA_LIMIT || "5");
const githubSearchApiUrl =
  process.env.MCP_PILOT_GITHUB_API_URL || "https://api.github.com/search/repositories";
const githubSearchLimit = Number(process.env.MCP_PILOT_GITHUB_LIMIT || "5");
const githubRepoApiUrl =
  process.env.MCP_PILOT_GITHUB_REPO_API_URL || "https://api.github.com/repos";
const githubRepoOwner = process.env.MCP_PILOT_GITHUB_REPO_OWNER || "";
const githubRepoName = process.env.MCP_PILOT_GITHUB_REPO_NAME || "";
const githubRepoRef = process.env.MCP_PILOT_GITHUB_REPO_REF || "main";
const githubRepoLimit = Number(process.env.MCP_PILOT_GITHUB_REPO_LIMIT || "5");
const githubRepoSnippetLength = Number(process.env.MCP_PILOT_GITHUB_REPO_SNIPPET_LENGTH || "240");
const githubToken = process.env.MCP_PILOT_GITHUB_TOKEN || "";
const larkCliBin = process.env.MCP_PILOT_LARK_CLI_BIN || "lark-cli";
const larkIdentity = process.env.MCP_PILOT_LARK_AS || "user";
const larkUserOpenId = process.env.MCP_PILOT_LARK_USER_OPEN_ID || "";
const larkChatId = process.env.MCP_PILOT_LARK_CHAT_ID || "";
const larkMessageLimit = Number(process.env.MCP_PILOT_LARK_MESSAGE_LIMIT || "10");
const larkDocsPageSize = Number(process.env.MCP_PILOT_LARK_DOCS_PAGE_SIZE || "10");
const larkSnippetLength = Number(process.env.MCP_PILOT_LARK_SNIPPET_LENGTH || "240");
const larkTimeoutMs = Number(process.env.MCP_PILOT_LARK_TIMEOUT_MS || "10000");

function writeJson(response, statusCode, body) {
  response.writeHead(statusCode, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

function checkAuth(request) {
  if (!controllerToken || request.url?.startsWith("/v1/") || request.url === "/healthz") {
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

function githubHeaders(accept = "application/vnd.github+json") {
  const headers = {
    "user-agent": "mandoforge-whiskey-mcp-pilot/1.0",
    accept,
  };
  if (githubToken) {
    headers.authorization = `Bearer ${githubToken}`;
  }
  return headers;
}

function normalizeSnippet(text) {
  const compact = String(text || "").replace(/\s+/g, " ").trim();
  if (compact.length <= githubRepoSnippetLength) {
    return compact;
  }
  return `${compact.slice(0, Math.max(0, githubRepoSnippetLength - 3)).trimEnd()}...`;
}

function encodeGitHubPath(path) {
  return String(path)
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

async function searchWikimedia(query) {
  const url = new URL(wikimediaApiUrl);
  url.searchParams.set("action", "opensearch");
  url.searchParams.set("search", query);
  url.searchParams.set("limit", String(Math.max(1, wikimediaLimit)));
  url.searchParams.set("namespace", "0");
  url.searchParams.set("format", "json");
  const response = await fetch(url, {
    headers: { "user-agent": "mandoforge-whiskey-mcp-pilot/1.0" },
  });
  if (!response.ok) {
    throw new Error(`wikimedia search failed with status ${response.status}`);
  }
  const body = await response.json();
  const titles = Array.isArray(body?.[1]) ? body[1] : [];
  const descriptions = Array.isArray(body?.[2]) ? body[2] : [];
  const urls = Array.isArray(body?.[3]) ? body[3] : [];
  return titles.map((title, index) => ({
    title,
    url: urls[index] || null,
    snippet: descriptions[index] || "",
    source_id: urls[index] || title,
    reference: title,
    retrieval_actor: "wikimedia-opensearch",
  }));
}

async function searchGitHubRepositories(query) {
  const url = new URL(githubSearchApiUrl);
  url.searchParams.set("q", query);
  url.searchParams.set("per_page", String(Math.max(1, githubSearchLimit)));
  const response = await fetch(url, { headers: githubHeaders() });
  if (!response.ok) {
    throw new Error(`github repository search failed with status ${response.status}`);
  }
  const body = await response.json();
  const items = Array.isArray(body?.items) ? body.items : [];
  return items.map((item) => ({
    title: item.full_name || item.name || "unknown",
    url: item.html_url || item.url || null,
    snippet: item.description || "",
    source_id: item.full_name || item.html_url || item.url || item.name || "unknown",
    reference: item.full_name || item.name || "unknown",
    retrieval_actor: "github-repository-search",
  }));
}

function scoreGitHubRepoPath(path, query) {
  const normalizedPath = path.toLowerCase();
  const normalizedQuery = query.trim().toLowerCase();
  const basename = normalizedPath.split("/").pop() || normalizedPath;
  let score = 0;
  if (normalizedPath.includes(normalizedQuery)) {
    score += 200;
  }
  if (basename.includes(normalizedQuery)) {
    score += 80;
  }
  if (normalizedPath.startsWith("docs/")) {
    score += 40;
  }
  if (normalizedPath.endsWith(".md")) {
    score += 20;
  }
  score -= normalizedPath.length / 1000;
  return score;
}

async function searchGitHubRepositoryContents(query) {
  if (!githubRepoOwner || !githubRepoName) {
    throw new Error("github repo contents mode requires repo owner and name");
  }

  const treeUrl = new URL(
    `${githubRepoApiUrl}/${encodeURIComponent(githubRepoOwner)}/${encodeURIComponent(
      githubRepoName,
    )}/git/trees/${encodeURIComponent(githubRepoRef)}`,
  );
  treeUrl.searchParams.set("recursive", "1");

  const treeResponse = await fetch(treeUrl, { headers: githubHeaders() });
  if (!treeResponse.ok) {
    throw new Error(`github repo tree fetch failed with status ${treeResponse.status}`);
  }
  const treeBody = await treeResponse.json();
  const tree = Array.isArray(treeBody?.tree) ? treeBody.tree : [];
  const normalizedQuery = String(query || "").trim().toLowerCase();
  const matchingFiles = tree
    .filter((item) => item?.type === "blob" && typeof item.path === "string")
    .filter((item) => {
      if (!normalizedQuery) {
        return true;
      }
      return item.path.toLowerCase().includes(normalizedQuery);
    })
    .sort((left, right) => {
      const scoreDiff =
        scoreGitHubRepoPath(right.path, normalizedQuery) -
        scoreGitHubRepoPath(left.path, normalizedQuery);
      if (scoreDiff !== 0) {
        return scoreDiff;
      }
      return left.path.localeCompare(right.path);
    })
    .slice(0, Math.max(1, githubRepoLimit));

  if (matchingFiles.length === 0) {
    throw new Error(`github repo contents search returned no path matches for query "${query}"`);
  }

  const items = await Promise.all(
    matchingFiles.map(async (item) => {
      let snippet = `Matched repository file ${item.path}`;
      if (item.url) {
        const blobResponse = await fetch(item.url, {
          headers: githubHeaders("application/vnd.github.raw+json"),
        });
        if (!blobResponse.ok) {
          throw new Error(
            `github repo blob fetch failed for ${item.path} with status ${blobResponse.status}`,
          );
        }
        snippet = normalizeSnippet(await blobResponse.text()) || snippet;
      }
      return {
        title: item.path,
        url: `https://github.com/${githubRepoOwner}/${githubRepoName}/blob/${encodeURIComponent(
          githubRepoRef,
        )}/${encodeGitHubPath(item.path)}`,
        snippet,
        source_id: `${githubRepoOwner}/${githubRepoName}:${item.path}@${githubRepoRef}`,
        reference: `${githubRepoOwner}/${githubRepoName}:${item.path}`,
        retrieval_actor: "github-repo-contents",
      };
    }),
  );

  return items;
}

function normalizeLarkMessageText(text) {
  return normalizeSnippet(String(text || "").replace(/\r/g, ""));
}

function stripHighlightTags(text) {
  return String(text || "").replace(/<\/?h[b]?>/g, "");
}

async function searchLarkChatMessages(query) {
  if (!larkUserOpenId && !larkChatId) {
    throw new Error("lark chat messages mode requires a user open id or chat id");
  }

  const args = [
    "im",
    "+chat-messages-list",
    "--as",
    larkIdentity,
    "--page-size",
    String(Math.max(1, larkMessageLimit)),
  ];
  if (larkChatId) {
    args.push("--chat-id", larkChatId);
  } else {
    args.push("--user-id", larkUserOpenId);
  }

  const { stdout } = await execFileAsync(larkCliBin, args, {
    timeout: larkTimeoutMs,
    maxBuffer: 1024 * 1024,
  });
  const response = JSON.parse(stdout || "{}");
  if (response.ok !== true) {
    throw new Error(response.error?.message || "lark chat messages request failed");
  }
  const messages = Array.isArray(response.data?.messages) ? response.data.messages : [];
  const normalizedQuery = String(query || "").trim().toLowerCase();
  const matchingMessages = messages.filter((message) => {
    if (!normalizedQuery) {
      return true;
    }
    return String(message.content || "").toLowerCase().includes(normalizedQuery);
  });

  if (matchingMessages.length === 0) {
    throw new Error(`lark chat messages search returned no matches for query "${query}"`);
  }

  return matchingMessages.slice(0, Math.max(1, larkMessageLimit)).map((message) => ({
    title: `${message.sender?.name || "Lark"} message ${message.message_id || "unknown"}`,
    url: message.message_app_link || null,
    snippet: normalizeLarkMessageText(message.content || "").slice(0, larkSnippetLength),
    source_id: message.message_id || message.message_position || "unknown",
    reference: `${message.chat_id || "unknown"}:${message.message_position || "unknown"}`,
    retrieval_actor: "lark-chat-messages",
  }));
}

async function searchLarkDocs(query) {
  const args = [
    "docs",
    "+search",
    "--as",
    larkIdentity,
    "--query",
    String(query || ""),
    "--page-size",
    String(Math.max(1, Math.min(20, larkDocsPageSize))),
    "--format",
    "json",
  ];

  const { stdout } = await execFileAsync(larkCliBin, args, {
    timeout: larkTimeoutMs,
    maxBuffer: 1024 * 1024,
  });
  const response = JSON.parse(stdout || "{}");
  if (response.ok !== true) {
    throw new Error(response.error?.message || "lark docs search request failed");
  }

  const items = Array.isArray(response.data?.results)
    ? response.data.results
    : Array.isArray(response.data?.items)
      ? response.data.items
      : [];
  if (items.length === 0) {
    throw new Error(`lark docs search returned no matches for query "${query}"`);
  }

  return items.map((item, index) => {
    const resultMeta = item.result_meta || {};
    const title =
      stripHighlightTags(item.title || item.title_highlighted || item.node_title || item.name) ||
      `Lark document ${index + 1}`;
    const summary = stripHighlightTags(
      item.summary || item.summary_highlighted || item.description || item.preview || "",
    );
    const docType =
      resultMeta.doc_types ||
      item.obj_type ||
      item.type ||
      item.node_type ||
      "unknown";
    const url =
      resultMeta.url || item.url || item.open_url || item.document_url || item.doc_url || null;
    const sourceId =
      resultMeta.token ||
      item.token ||
      item.obj_token ||
      item.node_token ||
      url ||
      `${docType}:${title}`;

    return {
      title,
      url,
      snippet: normalizeSnippet(summary || title).slice(0, larkSnippetLength),
      source_id: sourceId,
      reference: `${docType}:${sourceId}`,
      retrieval_actor: "lark-docs-search",
    };
  });
}

function pilotItems(query) {
  return [
    {
      title: "Whiskey MCP pilot",
      url: "https://example.invalid/mandoforge/whiskey-mcp-pilot",
      snippet: `Pilot response for ${query}`,
      source_id: "whiskey-mcp-pilot",
      reference: "Whiskey MCP pilot",
      retrieval_actor: "whiskey-mcp-pilot",
    },
  ];
}

function validateConnectors(payload) {
  const connectors = Array.isArray(payload.connectors) ? payload.connectors : [];
  const connectorNames = connectors.map((connector) => connector.name).filter(Boolean);
  const unhealthy = connectors.filter((connector) => connector.healthy !== true);
  const hasAllowedServer = connectorNames.includes(allowedServer);
  return {
    connectors,
    unhealthy,
    hasAllowedServer,
    checks: [
      step("connector_count", connectors.length > 0 ? "passed" : "failed", {
        server_count: connectors.length,
      }),
      step("allowed_server_present", hasAllowedServer ? "passed" : "failed", {
        allowed_server: allowedServer,
        connector_names: connectorNames,
      }),
      step("all_connectors_healthy", unhealthy.length === 0 ? "passed" : "failed", {
        unhealthy_count: unhealthy.length,
      }),
    ],
  };
}

function sourceAuthMode() {
  if (upstreamMode === "github_repositories" || upstreamMode === "github_repo_contents") {
    return githubToken ? "authenticated" : "anonymous";
  }
  if (upstreamMode === "lark_chat_messages" || upstreamMode === "lark_docs_search") {
    return "authenticated";
  }
  return "not_applicable";
}

function resolveGatewayQuery(payload) {
  if (Object.prototype.hasOwnProperty.call(payload.args || {}, "query")) {
    return payload.args.query;
  }
  if (Object.prototype.hasOwnProperty.call(payload.arguments || {}, "query")) {
    return payload.arguments.query;
  }
  if (Object.prototype.hasOwnProperty.call(payload.input || {}, "query")) {
    return payload.input.query;
  }
  return "OpenAI";
}

async function handleGatewayCall(request, response) {
  const payload = await readJson(request);
  if (payload.server !== allowedServer) {
    writeJson(response, 403, {
      result: {
        status: "blocked",
        message: `server ${payload.server || "unknown"} is not allowed`,
      },
    });
    return;
  }
  if (payload.tool !== "search") {
    writeJson(response, 404, {
      result: {
        status: "not_found",
        message: `tool ${payload.tool || "unknown"} is not available`,
      },
    });
    return;
  }
  const query = resolveGatewayQuery(payload);
  const items =
    upstreamMode === "wikimedia"
      ? await searchWikimedia(String(query))
      : upstreamMode === "github_repositories"
        ? await searchGitHubRepositories(String(query))
        : upstreamMode === "github_repo_contents"
          ? await searchGitHubRepositoryContents(String(query))
          : upstreamMode === "lark_chat_messages"
            ? await searchLarkChatMessages(String(query))
            : upstreamMode === "lark_docs_search"
              ? await searchLarkDocs(String(query))
        : pilotItems(String(query));
  writeJson(response, 200, {
    result: {
      status: "ok",
      source:
        upstreamMode === "wikimedia"
          ? "wikimedia-opensearch"
          : upstreamMode === "github_repositories"
            ? githubToken
              ? "github-repository-search-authenticated"
              : "github-repository-search"
            : upstreamMode === "github_repo_contents"
              ? githubToken
                ? "github-repo-contents-authenticated"
                : "github-repo-contents"
            : upstreamMode === "lark_chat_messages"
                ? "lark-chat-messages-authenticated"
            : upstreamMode === "lark_docs_search"
                ? "lark-docs-search-authenticated"
            : "whiskey-mcp-pilot",
      auth_mode: sourceAuthMode(),
      query,
      repository:
        upstreamMode === "github_repo_contents" && githubRepoOwner && githubRepoName
          ? `${githubRepoOwner}/${githubRepoName}@${githubRepoRef}`
          : null,
      chat_target:
        upstreamMode === "lark_chat_messages"
          ? larkChatId || larkUserOpenId || null
          : null,
      docs_search_scope_required:
        upstreamMode === "lark_docs_search" ? "search:docs:read" : null,
      item_count: items.length,
      items,
    },
  });
}

async function handleDeploymentValidation(request, response) {
  const payload = await readJson(request);
  if (payload.type !== "mandoforge.mcp_connector_deployment") {
    writeJson(response, 422, {
      status: "failed",
      message: "unexpected deployment payload type",
      steps: [step("payload_type", "failed", { observed: payload.type || null })],
    });
    return;
  }
  const validation = validateConnectors(payload);
  const failed = validation.checks.some((check) => check.status === "failed");
  writeJson(response, failed ? 502 : 200, {
    status: failed ? "failed" : "validated",
    deployment_id: `whiskey-mcp-deployment-${Date.now()}`,
    message: failed
      ? "Whiskey MCP deployment validation found an invalid connector state"
      : "Whiskey MCP connector deployment validated",
    steps: validation.checks,
  });
}

async function handleRolloutApproval(request, response) {
  const payload = await readJson(request);
  if (payload.type !== "mandoforge.mcp_connector_rollout") {
    writeJson(response, 422, {
      status: "failed",
      message: "unexpected rollout payload type",
      steps: [step("payload_type", "failed", { observed: payload.type || null })],
    });
    return;
  }
  const approved = payload.server_name === allowedServer && payload.rollout?.id;
  writeJson(response, approved ? 200 : 502, {
    status: approved ? "approved" : "failed",
    deployment_id: `whiskey-mcp-rollout-${Date.now()}`,
    message: approved
      ? "Whiskey MCP rollout approved"
      : "Whiskey MCP rollout was not approved",
    steps: [
      step("server_allowed", payload.server_name === allowedServer ? "passed" : "failed", {
        server_name: payload.server_name || "unknown",
        allowed_server: allowedServer,
      }),
      step("rollout_id_present", payload.rollout?.id ? "passed" : "failed"),
    ],
  });
}

async function handleRollbackValidation(request, response) {
  const payload = await readJson(request);
  if (payload.type !== "mandoforge.mcp_connector_rollout_rollback") {
    writeJson(response, 422, {
      status: "failed",
      message: "unexpected rollback payload type",
      steps: [step("payload_type", "failed", { observed: payload.type || null })],
    });
    return;
  }
  const rollbackable = payload.server_name === allowedServer && payload.rollout?.id;
  writeJson(response, rollbackable ? 200 : 502, {
    status: rollbackable ? "rolled_back" : "blocked",
    rollback_id: `whiskey-mcp-rollback-${Date.now()}`,
    message: rollbackable
      ? "Whiskey MCP rollback validated"
      : "Whiskey MCP rollback was blocked",
    steps: [
      step("server_allowed", payload.server_name === allowedServer ? "passed" : "failed", {
        server_name: payload.server_name || "unknown",
        allowed_server: allowedServer,
      }),
      step("rollout_id_present", payload.rollout?.id ? "passed" : "failed"),
    ],
  });
}

const server = http.createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/healthz") {
      writeJson(response, 200, { status: "ok" });
      return;
    }
    if (request.method === "GET" && request.url === `/v1/servers/${allowedServer}/tools`) {
      writeJson(response, 200, [
        {
          name: "search",
          description: "Search the Whiskey MCP pilot corpus",
        },
      ]);
      return;
    }
    if (request.method === "POST" && request.url === "/v1/call") {
      await handleGatewayCall(request, response);
      return;
    }
    if (!checkAuth(request)) {
      writeJson(response, 401, { status: "unauthorized" });
      return;
    }
    if (request.method === "POST" && request.url === "/mcp/deployment/validate") {
      await handleDeploymentValidation(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/mcp/rollout/approve") {
      await handleRolloutApproval(request, response);
      return;
    }
    if (request.method === "POST" && request.url === "/mcp/rollback/validate") {
      await handleRollbackValidation(request, response);
      return;
    }
    writeJson(response, 404, { status: "not_found" });
  } catch (error) {
    writeJson(response, 502, {
      status: "failed",
      message: error.message || "Whiskey MCP pilot controller failed",
    });
  }
});

server.listen(listenPort, listenHost, () => {
  console.log(`MCP pilot controller listening on http://${listenHost}:${listenPort}`);
});
