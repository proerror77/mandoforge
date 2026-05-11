const state = {
  agents: [],
  session: null,
  events: [],
};

const agentRoot = document.querySelector("#agents");
const approvalRoot = document.querySelector("#approvals");
const eventRoot = document.querySelector("#events");
const reportRoot = document.querySelector("#final-report");
const titleRoot = document.querySelector("#session-title");
const statusRoot = document.querySelector("#session-status");

document.querySelector("#new-session").addEventListener("click", runDemo);

async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: { "content-type": "application/json" },
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
}

async function runDemo() {
  const agent = state.agents[0];
  const session = await api("/api/sessions", {
    method: "POST",
    body: JSON.stringify({
      agent_id: agent.id,
      title: "GMV decline diagnosis",
      message: "昨天 GMV 为什么下降？请找出主要原因，并生成今天可执行的运营建议。",
    }),
  });
  state.session = await api(`/api/sessions/${session.id}/run`, { method: "POST" });
  await refreshSession();
  await refreshApprovals();
}

async function refreshSession() {
  if (!state.session) return;
  state.session = await api(`/api/sessions/${state.session.id}`);
  state.events = await api(`/api/sessions/${state.session.id}/events`);
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
  const response = [...state.events].reverse().find((event) => event.event_type === "llm.response");
  if (!response) {
    reportRoot.textContent = "Run the demo to generate the GMV diagnostic report.";
    return;
  }
  const report = response.payload.final_report;
  reportRoot.textContent = [
    `GMV drop: ${report.gmv_drop}`,
    `Top abnormal SKUs: ${report.top_skus.join(", ")}`,
    "",
    "Drivers:",
    ...report.drivers.map((driver) => `- ${driver}`),
    "",
    "Recommendations:",
    ...report.recommendations.map((recommendation) => `- ${recommendation}`),
  ].join("\n");
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

