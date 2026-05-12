const state = {
  agents: [],
  session: null,
  events: [],
  artifacts: [],
  toolCalls: [],
  auditLogs: [],
  providers: [],
  evalRuns: [],
  usage: null,
  selectedArtifactId: null,
  selectedToolCallId: null,
  selectedAuditLogId: null,
};

const agentRoot = document.querySelector("#agents");
const approvalRoot = document.querySelector("#approvals");
const eventRoot = document.querySelector("#events");
const reportRoot = document.querySelector("#final-report");
const titleRoot = document.querySelector("#session-title");
const statusRoot = document.querySelector("#session-status");
const artifactRoot = document.querySelector("#artifacts");
const artifactDetailRoot = document.querySelector("#artifact-detail");
const toolCallRoot = document.querySelector("#tool-calls");
const toolDetailRoot = document.querySelector("#tool-detail");
const auditLogRoot = document.querySelector("#audit-logs");
const auditDetailRoot = document.querySelector("#audit-detail");
const providerRoot = document.querySelector("#providers");
const evalRunRoot = document.querySelector("#eval-runs");
const usageRoot = document.querySelector("#usage-summary");
const governanceRoot = document.querySelector("#governance-status");
const agentForm = document.querySelector("#agent-form");
const providerForm = document.querySelector("#provider-form");

document.querySelector("#new-session").addEventListener("click", runDemo);
agentForm.addEventListener("submit", createAgent);
providerForm.addEventListener("submit", createProvider);

async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: {
      "content-type": "application/json",
      "x-mandoforge-subject": "web-admin",
      "x-mandoforge-roles": "admin",
    },
    ...options,
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return response.json();
}

async function boot() {
  state.agents = await api("/api/agents");
  renderAgents();
  await refreshApprovals();
  await refreshOps();
}

async function createAgent(event) {
  event.preventDefault();
  const form = new FormData(agentForm);
  const agent = await api("/api/agents", {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      model: form.get("model"),
      system_prompt: form.get("system_prompt"),
      tools: String(form.get("tools") || "")
        .split(",")
        .map((tool) => tool.trim())
        .filter(Boolean),
    }),
  });
  state.agents = [agent, ...state.agents.filter((existing) => existing.id !== agent.id)];
  renderAgents();
}

async function createProvider(event) {
  event.preventDefault();
  const form = new FormData(providerForm);
  const dailyRequestLimit = Number(form.get("daily_request_limit") || 0);
  const perRequestCents = Number(form.get("per_request_cents") || 0);
  await api("/api/providers", {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      provider_type: form.get("provider_type"),
      default_model: form.get("default_model"),
      config: {
        budget: { daily_request_limit: dailyRequestLimit },
        pricing: { per_request_cents: perRequestCents },
      },
    }),
  });
  await refreshOps();
}

async function runDemo() {
  const agent = state.agents[0];
  const session = await api("/api/sessions", {
    method: "POST",
    body: JSON.stringify({
      agent_id: agent.id,
      title: "Generic runtime diagnostics",
      message:
        "Read README and config, query demo platform_events, request approval before shell or file write, and generate diagnostics.md.",
    }),
  });
  state.session = await api(`/api/sessions/${session.id}/run`, { method: "POST" });
  state.selectedArtifactId = null;
  state.selectedToolCallId = null;
  state.selectedAuditLogId = null;
  await refreshSession();
  await refreshApprovals();
  await refreshOps();
}

async function refreshOps() {
  const [providers, evalRuns, usage] = await Promise.all([
    api("/api/providers"),
    api("/api/eval/runs"),
    api("/api/usage"),
  ]);
  state.providers = providers;
  state.evalRuns = evalRuns;
  state.usage = usage;
  renderOps();
}

async function refreshSession() {
  if (!state.session) return;
  state.session = await api(`/api/sessions/${state.session.id}`);
  state.events = await api(`/api/sessions/${state.session.id}/events`);
  state.artifacts = await api(`/api/sessions/${state.session.id}/artifacts`);
  state.toolCalls = await api(`/api/sessions/${state.session.id}/tool-calls`);
  state.auditLogs = await api(`/api/sessions/${state.session.id}/audit-logs`);
  state.selectedArtifactId ??= state.artifacts[0]?.id ?? null;
  state.selectedToolCallId ??= state.toolCalls[0]?.id ?? null;
  state.selectedAuditLogId ??= state.auditLogs[0]?.id ?? null;
  renderSession();
}

async function refreshApprovals() {
  const approvals = await api("/api/approvals");
  approvalRoot.innerHTML = approvals.length
    ? approvals.map(renderApproval).join("")
    : `<div class="muted">No pending approvals</div>`;
  approvalRoot.querySelectorAll("[data-approve]").forEach((button) => {
    button.addEventListener("click", () => decide(button.dataset.approve, "approve"));
  });
  approvalRoot.querySelectorAll("[data-reject]").forEach((button) => {
    button.addEventListener("click", () => decide(button.dataset.reject, "reject"));
  });
}

async function decide(id, decision) {
  await api(`/api/approvals/${id}/${decision}`, { method: "POST" });
  await refreshApprovals();
  await refreshSession();
  await refreshOps();
}

function renderOps() {
  renderUsage();
  renderProviders();
  renderEvalRuns();
  governanceRoot.innerHTML = `
    <dl>
      <dt>Policy</dt>
      <dd>YAML policy enforced through Tool Router</dd>
      <dt>Vault</dt>
      <dd>Vault references fail closed unless the Vault provider is configured</dd>
      <dt>Workers</dt>
      <dd>Execution queue and worker drain APIs are enabled</dd>
      <dt>MCP</dt>
      <dd>Gateway calls require global and team-scoped allowlists</dd>
    </dl>
  `;
}

function renderUsage() {
  const usage = state.usage;
  if (!usage) {
    usageRoot.innerHTML = `<div class="muted">Usage data is not loaded.</div>`;
    return;
  }
  usageRoot.innerHTML = `
    <dl>
      <dt>Sessions</dt>
      <dd>${usage.session_count}</dd>
      <dt>Provider requests</dt>
      <dd>${usage.provider_request_count}</dd>
      <dt>Tool calls</dt>
      <dd>${usage.tool_call_count} total · ${usage.tool_success_count} completed · ${usage.tool_failed_count} failed</dd>
      <dt>Approval records</dt>
      <dd>${usage.approval_count}</dd>
      <dt>Estimated provider cost</dt>
      <dd>${Number(usage.estimated_provider_cost_cents || 0).toFixed(2)} cents</dd>
    </dl>
  `;
}

function renderProviders() {
  providerRoot.innerHTML = state.providers.length
    ? state.providers
        .map(
          (provider) => `
            <div class="item">
              <strong>${escapeHtml(provider.name)}</strong>
              <div class="muted">${escapeHtml(provider.provider_type)} · ${escapeHtml(provider.status)}</div>
              <pre>${escapeHtml(JSON.stringify(provider.config, null, 2))}</pre>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No stored providers</div>`;
}

function renderEvalRuns() {
  evalRunRoot.innerHTML = state.evalRuns.length
    ? state.evalRuns
        .map(
          (run) => `
            <div class="item">
              <strong>${escapeHtml(run.status)} · score ${escapeHtml(run.score ?? "n/a")}</strong>
              <div class="muted">${escapeHtml(run.created_at)}</div>
              <pre>${escapeHtml(JSON.stringify(run.details, null, 2))}</pre>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No eval runs</div>`;
}

function renderAgents() {
  agentRoot.innerHTML = state.agents
    .map(
      (agent) => `
        <div class="item">
          <strong>${escapeHtml(agent.name)}</strong>
          <div class="muted">${escapeHtml(agent.kind)} · ${escapeHtml(agent.provider)} · ${escapeHtml(agent.model)}</div>
        </div>
      `,
    )
    .join("");
}

function renderSession() {
  titleRoot.textContent = state.session.title;
  statusRoot.textContent = state.session.status;
  eventRoot.innerHTML = state.events.map(renderEvent).join("");
  renderArtifacts();
  renderToolCalls();
  renderAuditLogs();
  const response = [...state.events].reverse().find((event) => event.event_type === "llm.response");
  if (!response) {
    reportRoot.textContent = "Run the demo to generate the runtime diagnostics report.";
    return;
  }
  const report = response.payload.final_report;
  reportRoot.textContent = [
    report.summary,
    "",
    `Files read: ${report.files_read.join(", ")}`,
    `SQL tables: ${report.sql_tables.join(", ")}`,
    `Policy events: ${report.policy_events.join(", ")}`,
    `Artifacts: ${report.artifacts.join(", ")}`,
    "",
    "Next steps:",
    ...report.next_steps.map((step) => `- ${step}`),
  ].join("\n");
}

function renderAuditLogs() {
  auditLogRoot.innerHTML = state.auditLogs.length
    ? state.auditLogs.map(renderAuditLog).join("")
    : `<div class="muted">No audit logs yet</div>`;
  auditLogRoot.querySelectorAll("[data-audit-log]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedAuditLogId = button.dataset.auditLog;
      renderAuditLogs();
    });
  });

  const selected = state.auditLogs.find((log) => log.id === state.selectedAuditLogId);
  auditDetailRoot.innerHTML = selected
    ? `
      <strong>${escapeHtml(selected.action)}</strong>
      <div class="muted">${escapeHtml(selected.actor_type)} · ${escapeHtml(selected.resource_type)} · ${escapeHtml(selected.created_at)}</div>
      <dl>
        <dt>Resource</dt>
        <dd><pre>${escapeHtml(JSON.stringify({ resource_id: selected.resource_id, actor_id: selected.actor_id }, null, 2))}</pre></dd>
        <dt>Details</dt>
        <dd><pre>${escapeHtml(JSON.stringify(selected.details, null, 2))}</pre></dd>
      </dl>
    `
    : `<div class="muted">Select an audit log to inspect details.</div>`;
}

function renderArtifacts() {
  artifactRoot.innerHTML = state.artifacts.length
    ? state.artifacts.map(renderArtifact).join("")
    : `<div class="muted">No artifacts yet</div>`;
  artifactRoot.querySelectorAll("[data-artifact]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedArtifactId = button.dataset.artifact;
      renderArtifacts();
    });
  });

  const selected = state.artifacts.find((artifact) => artifact.id === state.selectedArtifactId);
  artifactDetailRoot.innerHTML = selected
    ? `
      <strong>${escapeHtml(selected.name)}</strong>
      <div class="muted">${escapeHtml(selected.artifact_type)} · ${escapeHtml(selected.created_at)}</div>
      <pre>${escapeHtml(JSON.stringify(selected.content, null, 2))}</pre>
    `
    : `<div class="muted">Select an artifact to inspect its content.</div>`;
}

function renderToolCalls() {
  toolCallRoot.innerHTML = state.toolCalls.length
    ? state.toolCalls.map(renderToolCall).join("")
    : `<div class="muted">No tool calls yet</div>`;
  toolCallRoot.querySelectorAll("[data-tool-call]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedToolCallId = button.dataset.toolCall;
      renderToolCalls();
    });
  });

  const selected = state.toolCalls.find((call) => call.id === state.selectedToolCallId);
  toolDetailRoot.innerHTML = selected
    ? `
      <strong>${escapeHtml(selected.tool_name)}</strong>
      <div class="muted">${escapeHtml(selected.status)} · ${escapeHtml(selected.risk_level)}</div>
      <dl>
        <dt>Policy decision</dt>
        <dd><pre>${escapeHtml(JSON.stringify(selected.policy_decision, null, 2))}</pre></dd>
        <dt>Arguments</dt>
        <dd><pre>${escapeHtml(JSON.stringify(selected.args, null, 2))}</pre></dd>
        <dt>Result</dt>
        <dd><pre>${escapeHtml(JSON.stringify(selected.result ?? {}, null, 2))}</pre></dd>
      </dl>
    `
    : `<div class="muted">Select a tool call to inspect policy and result details.</div>`;
}

function renderEvent(event) {
  return `
    <article class="event">
      <strong>#${event.seq} ${escapeHtml(event.event_type)}</strong>
      <pre>${escapeHtml(JSON.stringify(event.payload, null, 2))}</pre>
    </article>
  `;
}

function renderApproval(approval) {
  const isPending = approval.status === "pending";
  return `
    <div class="item">
      <strong>${escapeHtml(approval.action)}</strong>
      <div class="muted">${escapeHtml(approval.risk_level)} · ${escapeHtml(approval.status)}</div>
      <p>${escapeHtml(approval.reason)}</p>
      ${
        isPending
          ? `<button class="secondary" data-approve="${approval.id}">Approve</button><button class="secondary reject" data-reject="${approval.id}">Reject</button>`
          : ""
      }
    </div>
  `;
}

function renderArtifact(artifact) {
  const selected = artifact.id === state.selectedArtifactId ? " selected" : "";
  return `
    <button class="item-button${selected}" data-artifact="${artifact.id}">
      <strong>${escapeHtml(artifact.name)}</strong>
      <span>${escapeHtml(artifact.artifact_type)}</span>
    </button>
  `;
}

function renderToolCall(call) {
  const selected = call.id === state.selectedToolCallId ? " selected" : "";
  return `
    <button class="item-button${selected}" data-tool-call="${call.id}">
      <strong>${escapeHtml(call.tool_name)}</strong>
      <span>${escapeHtml(call.status)} · ${escapeHtml(call.risk_level)}</span>
    </button>
  `;
}

function renderAuditLog(log) {
  const selected = log.id === state.selectedAuditLogId ? " selected" : "";
  return `
    <button class="item-button${selected}" data-audit-log="${log.id}">
      <strong>${escapeHtml(log.action)}</strong>
      <span>${escapeHtml(log.actor_type)} · ${escapeHtml(log.resource_type)}</span>
    </button>
  `;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

boot().catch((error) => {
  reportRoot.textContent = error.message;
});
