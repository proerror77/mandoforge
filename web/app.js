const state = {
  agents: [],
  session: null,
  events: [],
  artifacts: [],
  toolCalls: [],
  auditLogs: [],
  providers: [],
  evalDatasets: [],
  evalCases: [],
  evalRuns: [],
  evalGates: {},
  mcpServers: [],
  mcpTeamId: "",
  executionJobs: [],
  usageRollups: [],
  usage: null,
  organizations: [],
  teams: [],
  projects: [],
  memberships: [],
  selectedOrganizationId: "",
  selectedTeamId: "",
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
const evalDatasetRoot = document.querySelector("#eval-datasets");
const evalCaseRoot = document.querySelector("#eval-cases");
const evalRunRoot = document.querySelector("#eval-runs");
const mcpServerRoot = document.querySelector("#mcp-servers");
const executionJobRoot = document.querySelector("#execution-jobs");
const usageRoot = document.querySelector("#usage-summary");
const usageRollupRoot = document.querySelector("#usage-rollups");
const governanceRoot = document.querySelector("#governance-status");
const organizationRoot = document.querySelector("#organizations");
const teamRoot = document.querySelector("#teams");
const projectRoot = document.querySelector("#projects");
const membershipRoot = document.querySelector("#memberships");
const agentForm = document.querySelector("#agent-form");
const organizationForm = document.querySelector("#organization-form");
const teamForm = document.querySelector("#team-form");
const projectForm = document.querySelector("#project-form");
const membershipForm = document.querySelector("#membership-form");
const providerForm = document.querySelector("#provider-form");
const evalDatasetForm = document.querySelector("#eval-dataset-form");
const evalCaseForm = document.querySelector("#eval-case-form");
const evalRunForm = document.querySelector("#eval-run-form");
const mcpForm = document.querySelector("#mcp-form");
const loadMcpButton = document.querySelector("#load-mcp");
const loadEvalCasesButton = document.querySelector("#load-eval-cases");
const refreshExecutionJobsButton = document.querySelector("#refresh-execution-jobs");
const createUsageRollupButton = document.querySelector("#create-usage-rollup");

document.querySelector("#new-session").addEventListener("click", runDemo);
agentForm.addEventListener("submit", createAgent);
organizationForm.addEventListener("submit", createOrganization);
teamForm.addEventListener("submit", createTeam);
projectForm.addEventListener("submit", createProject);
membershipForm.addEventListener("submit", createMembership);
providerForm.addEventListener("submit", createProvider);
evalDatasetForm.addEventListener("submit", createEvalDataset);
evalCaseForm.addEventListener("submit", createEvalCase);
evalRunForm.addEventListener("submit", createEvalRun);
mcpForm.addEventListener("submit", createMcpServer);
loadMcpButton.addEventListener("click", loadMcpServers);
loadEvalCasesButton.addEventListener("click", loadEvalCases);
refreshExecutionJobsButton.addEventListener("click", refreshExecutionJobs);
createUsageRollupButton.addEventListener("click", createUsageRollup);

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

async function createOrganization(event) {
  event.preventDefault();
  const form = new FormData(organizationForm);
  const organization = await api("/api/organizations", {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      slug: form.get("slug"),
    }),
  });
  setOrganizationId(organization.id);
  await refreshOps();
}

async function createTeam(event) {
  event.preventDefault();
  const form = new FormData(teamForm);
  const organizationId = String(form.get("organization_id") || state.selectedOrganizationId).trim();
  const team = await api(`/api/organizations/${organizationId}/teams`, {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      slug: form.get("slug"),
    }),
  });
  setOrganizationId(organizationId);
  setTeamId(team.id);
  await refreshOps();
}

async function createProject(event) {
  event.preventDefault();
  const form = new FormData(projectForm);
  const teamId = String(form.get("team_id") || state.selectedTeamId).trim();
  await api(`/api/teams/${teamId}/projects`, {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      slug: form.get("slug"),
    }),
  });
  setTeamId(teamId);
  await refreshOps();
}

async function createMembership(event) {
  event.preventDefault();
  const form = new FormData(membershipForm);
  const organizationId = String(
    form.get("organization_id") || state.selectedOrganizationId,
  ).trim();
  const payload = {
    user_id: String(form.get("user_id") || "").trim(),
    role: String(form.get("role") || "").trim(),
  };
  const teamId = String(form.get("team_id") || "").trim();
  const projectId = String(form.get("project_id") || "").trim();
  if (teamId) payload.team_id = teamId;
  if (projectId) payload.project_id = projectId;
  await api(`/api/organizations/${organizationId}/memberships`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
  setOrganizationId(organizationId);
  await refreshOps();
}

async function createProvider(event) {
  event.preventDefault();
  const form = new FormData(providerForm);
  const dailyRequestLimit = Number(form.get("daily_request_limit") || 0);
  const dailyCostLimitCents = Number(form.get("daily_cost_limit_cents") || 0);
  const perRequestCents = Number(form.get("per_request_cents") || 0);
  const promptTokenCents = Number(form.get("per_1k_prompt_tokens_cents") || 0);
  const completionTokenCents = Number(form.get("per_1k_completion_tokens_cents") || 0);
  await api("/api/providers", {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      provider_type: form.get("provider_type"),
      default_model: form.get("default_model"),
      config: {
        budget: {
          daily_request_limit: dailyRequestLimit,
          daily_cost_limit_cents: dailyCostLimitCents,
        },
        pricing: {
          per_request_cents: perRequestCents,
          per_1k_prompt_tokens_cents: promptTokenCents,
          per_1k_completion_tokens_cents: completionTokenCents,
        },
      },
    }),
  });
  await refreshOps();
}

async function createEvalDataset(event) {
  event.preventDefault();
  const form = new FormData(evalDatasetForm);
  const dataset = await api("/api/eval/datasets", {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      description: form.get("description"),
    }),
  });
  setEvalDatasetId(dataset.id);
  await refreshOps();
}

async function createEvalCase(event) {
  event.preventDefault();
  const form = new FormData(evalCaseForm);
  const datasetId = String(form.get("dataset_id") || "").trim();
  await api(`/api/eval/datasets/${datasetId}/cases`, {
    method: "POST",
    body: JSON.stringify({
      input: parseJsonField(form.get("input"), "Input JSON"),
      expected: parseJsonField(form.get("expected"), "Expected JSON"),
      grading_policy: parseJsonField(form.get("grading_policy"), "Grading policy JSON"),
    }),
  });
  setEvalDatasetId(datasetId);
  await loadEvalCases();
}

async function createEvalRun(event) {
  event.preventDefault();
  const form = new FormData(evalRunForm);
  const datasetId = String(form.get("dataset_id") || "").trim();
  await api(`/api/eval/datasets/${datasetId}/runs`, {
    method: "POST",
    body: JSON.stringify({
      agent_id: String(form.get("agent_id") || "").trim(),
    }),
  });
  setEvalDatasetId(datasetId);
  await refreshOps();
}

async function gateEvalRun(runId) {
  const decision = await api(`/api/eval/runs/${runId}/gate`, {
    method: "POST",
    body: JSON.stringify({ min_score: 1.0, require_completed: true }),
  });
  state.evalGates[runId] = decision;
  renderEvalRuns();
}

async function createMcpServer(event) {
  event.preventDefault();
  const form = new FormData(mcpForm);
  const teamId = String(form.get("team_id") || "").trim();
  const toolAllowlist = String(form.get("tool_allowlist") || "")
    .split(",")
    .map((tool) => tool.trim())
    .filter(Boolean);
  await api(`/api/teams/${teamId}/mcp-servers`, {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      transport: form.get("transport"),
      tool_allowlist: toolAllowlist,
    }),
  });
  state.mcpTeamId = teamId;
  await loadMcpServers();
}

async function createUsageRollup() {
  await api("/api/usage/rollups", {
    method: "POST",
    body: JSON.stringify({}),
  });
  await refreshOps();
}

async function loadEvalCases() {
  const form = new FormData(evalCaseForm);
  const datasetId = String(form.get("dataset_id") || "").trim();
  if (!datasetId) {
    evalCaseRoot.innerHTML = `<div class="muted">Enter a dataset ID to load eval cases.</div>`;
    return;
  }
  setEvalDatasetId(datasetId);
  state.evalCases = await api(`/api/eval/datasets/${datasetId}/cases`);
  renderEvalCases();
}

async function loadMcpServers() {
  const form = new FormData(mcpForm);
  const teamId = String(form.get("team_id") || state.mcpTeamId || "").trim();
  if (!teamId) {
    mcpServerRoot.innerHTML = `<div class="muted">Enter a team ID to load MCP servers.</div>`;
    return;
  }
  state.mcpTeamId = teamId;
  state.mcpServers = await api(`/api/teams/${teamId}/mcp-servers`);
  renderMcpServers();
}

async function discoverMcpTools(serverId) {
  if (!state.mcpTeamId) return;
  await api(`/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}/discover`, {
    method: "POST",
  });
  await loadMcpServers();
}

async function refreshExecutionJobs() {
  state.executionJobs = await api("/api/execution-jobs");
  renderExecutionJobs();
}

async function runExecutionJob(jobId) {
  await api(`/api/execution-jobs/${jobId}/run`, { method: "POST" });
  await refreshExecutionJobs();
  await refreshApprovals();
  await refreshSession();
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
  const [providers, evalDatasets, evalRuns, usage, usageRollups, organizations, executionJobs] =
    await Promise.all([
      api("/api/providers"),
      api("/api/eval/datasets"),
      api("/api/eval/runs"),
      api("/api/usage"),
      api("/api/usage/rollups"),
      api("/api/organizations"),
      api("/api/execution-jobs"),
    ]);
  state.providers = providers;
  state.evalDatasets = evalDatasets;
  state.evalRuns = evalRuns;
  state.usage = usage;
  state.usageRollups = usageRollups;
  state.organizations = organizations;
  state.executionJobs = executionJobs;
  if (
    state.selectedOrganizationId &&
    !organizations.some((organization) => organization.id === state.selectedOrganizationId)
  ) {
    state.selectedOrganizationId = "";
    state.selectedTeamId = "";
  }
  if (!state.selectedOrganizationId && organizations[0]) {
    setOrganizationId(organizations[0].id);
  }
  if (state.selectedOrganizationId) {
    const [teams, memberships] = await Promise.all([
      api(`/api/organizations/${state.selectedOrganizationId}/teams`),
      api(`/api/organizations/${state.selectedOrganizationId}/memberships`),
    ]);
    state.teams = teams;
    state.memberships = memberships;
    if (state.selectedTeamId && !teams.some((team) => team.id === state.selectedTeamId)) {
      state.selectedTeamId = "";
    }
    if (!state.selectedTeamId && teams[0]) {
      setTeamId(teams[0].id);
    }
  } else {
    state.teams = [];
    state.memberships = [];
    state.selectedTeamId = "";
  }
  state.projects = state.selectedTeamId
    ? await api(`/api/teams/${state.selectedTeamId}/projects`)
    : [];
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
  approvalRoot.querySelectorAll("[data-expire]").forEach((button) => {
    button.addEventListener("click", () => decide(button.dataset.expire, "expire"));
  });
  approvalRoot.querySelectorAll("[data-approval-modify]").forEach((form) => {
    form.addEventListener("submit", (event) => modifyApproval(event, form.dataset.approvalModify));
  });
}

async function decide(id, decision) {
  await api(`/api/approvals/${id}/${decision}`, { method: "POST" });
  await refreshApprovals();
  await refreshSession();
  await refreshOps();
}

async function modifyApproval(event, id) {
  event.preventDefault();
  const form = new FormData(event.currentTarget);
  await api(`/api/approvals/${id}/modify`, {
    method: "POST",
    body: JSON.stringify({
      args: parseJsonField(form.get("args"), "Approval args JSON"),
      comment: String(form.get("comment") || "").trim() || null,
    }),
  });
  await refreshApprovals();
  await refreshSession();
  await refreshOps();
}

function renderOps() {
  renderUsage();
  renderTenantGovernance();
  renderProviders();
  renderEvalDatasets();
  renderEvalCases();
  renderEvalRuns();
  renderMcpServers();
  renderExecutionJobs();
  governanceRoot.innerHTML = `
    <dl>
      <dt>Policy</dt>
      <dd>YAML policy enforced through Tool Router</dd>
      <dt>Vault</dt>
      <dd>Vault references fail closed unless the Vault provider is configured</dd>
      <dt>Workers</dt>
      <dd>Execution queue and worker drain APIs are enabled</dd>
      <dt>MCP</dt>
      <dd>Gateway calls require global and team-scoped allowlists; discovery can import gateway tools into a team server allowlist</dd>
    </dl>
  `;
}

function renderExecutionJobs() {
  executionJobRoot.innerHTML = state.executionJobs.length
    ? state.executionJobs
        .map(
          (job) => `
            <div class="item">
              <strong>${escapeHtml(job.tool_name)}</strong>
              <div class="muted">${escapeHtml(job.status)} · ${escapeHtml(job.id)}</div>
              <div class="muted">Worker: ${escapeHtml(job.worker_id || "none")} · Lease: ${escapeHtml(job.lease_expires_at || "none")}</div>
              ${
                job.status === "queued"
                  ? `<button class="secondary" data-run-execution-job="${job.id}">Run Job</button>`
                  : ""
              }
              <pre>${escapeHtml(
                JSON.stringify(
                  {
                    session_id: job.session_id,
                    approval_id: job.approval_id,
                    tool_call_id: job.tool_call_id,
                    enqueued_at: job.enqueued_at,
                    started_at: job.started_at,
                    completed_at: job.completed_at,
                  },
                  null,
                  2,
                ),
              )}</pre>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No execution jobs</div>`;
  executionJobRoot.querySelectorAll("[data-run-execution-job]").forEach((button) => {
    button.addEventListener("click", () => runExecutionJob(button.dataset.runExecutionJob));
  });
}

function renderTenantGovernance() {
  organizationRoot.innerHTML = state.organizations.length
    ? state.organizations
        .map(
          (organization) => `
            <button class="item-button${organization.id === state.selectedOrganizationId ? " selected" : ""}" data-organization="${organization.id}">
              <strong>${escapeHtml(organization.name)}</strong>
              <span>${escapeHtml(organization.slug)} · ${escapeHtml(organization.id)}</span>
            </button>
          `,
        )
        .join("")
    : `<div class="muted">No organizations yet</div>`;
  organizationRoot.querySelectorAll("[data-organization]").forEach((button) => {
    button.addEventListener("click", async () => {
      setOrganizationId(button.dataset.organization);
      state.selectedTeamId = "";
      await refreshOps();
    });
  });

  teamRoot.innerHTML = state.selectedOrganizationId
    ? state.teams.length
      ? state.teams
          .map(
            (team) => `
              <button class="item-button${team.id === state.selectedTeamId ? " selected" : ""}" data-team="${team.id}">
                <strong>${escapeHtml(team.name)}</strong>
                <span>${escapeHtml(team.slug)} · ${escapeHtml(team.id)}</span>
              </button>
            `,
          )
          .join("")
      : `<div class="muted">No teams for selected organization</div>`
    : `<div class="muted">Select an organization to manage teams</div>`;
  teamRoot.querySelectorAll("[data-team]").forEach((button) => {
    button.addEventListener("click", async () => {
      setTeamId(button.dataset.team);
      await refreshOps();
    });
  });

  projectRoot.innerHTML = state.selectedTeamId
    ? state.projects.length
      ? state.projects
          .map(
            (project) => `
              <div class="item">
                <strong>${escapeHtml(project.name)}</strong>
                <div class="muted">${escapeHtml(project.slug)} · ${escapeHtml(project.id)}</div>
              </div>
            `,
          )
          .join("")
      : `<div class="muted">No projects for selected team</div>`
    : `<div class="muted">Select a team to manage projects</div>`;

  membershipRoot.innerHTML = state.selectedOrganizationId
    ? state.memberships.length
      ? state.memberships
          .map(
            (membership) => `
              <div class="item">
                <strong>${escapeHtml(membership.user_id)}</strong>
                <div class="muted">${escapeHtml(membership.role)} · ${escapeHtml(membership.id)}</div>
                <pre>${escapeHtml(
                  JSON.stringify(
                    {
                      team_id: membership.team_id,
                      project_id: membership.project_id,
                    },
                    null,
                    2,
                  ),
                )}</pre>
              </div>
            `,
          )
          .join("")
      : `<div class="muted">No memberships for selected organization</div>`
    : `<div class="muted">Select an organization to manage memberships</div>`;
}

function renderUsage() {
  const usage = state.usage;
  if (!usage) {
    usageRoot.innerHTML = `<div class="muted">Usage data is not loaded.</div>`;
    return;
  }
  const providerEntries = Object.entries(usage.by_provider || {}).sort(
    ([, left], [, right]) =>
      Number(right.estimated_cost_cents || 0) - Number(left.estimated_cost_cents || 0),
  );
  const toolEntries = Object.entries(usage.by_tool || {}).sort(
    ([, left], [, right]) => Number(right.call_count || 0) - Number(left.call_count || 0),
  );
  const averageToolDurationMs =
    usage.tool_call_count > 0 ? usage.total_tool_duration_ms / usage.tool_call_count : 0;
  usageRoot.innerHTML = `
    <div class="metric-grid">
      <div class="metric">
        <span>Provider cost</span>
        <strong>${formatCents(usage.estimated_provider_cost_cents)}</strong>
      </div>
      <div class="metric">
        <span>Provider tokens</span>
        <strong>${formatInteger(usage.total_tokens)}</strong>
      </div>
      <div class="metric">
        <span>Tool calls</span>
        <strong>${formatInteger(usage.tool_call_count)}</strong>
      </div>
      <div class="metric">
        <span>Avg tool runtime</span>
        <strong>${formatDurationMs(averageToolDurationMs)}</strong>
      </div>
    </div>
    <dl>
      <dt>Sessions</dt>
      <dd>${formatInteger(usage.session_count)} sessions · ${formatInteger(usage.event_count)} events</dd>
      <dt>Provider requests</dt>
      <dd>${formatInteger(usage.provider_request_count)} requests · ${formatInteger(usage.provider_response_count)} responses</dd>
      <dt>Provider tokens</dt>
      <dd>${formatInteger(usage.total_tokens)} total · ${formatInteger(usage.prompt_tokens)} prompt · ${formatInteger(usage.completion_tokens)} completion</dd>
      <dt>Tool calls</dt>
      <dd>${formatInteger(usage.tool_call_count)} total · ${formatInteger(usage.tool_success_count)} completed · ${formatInteger(usage.tool_failed_count)} failed</dd>
      <dt>Approval records</dt>
      <dd>${formatInteger(usage.approval_count)}</dd>
    </dl>
    <h4>Provider Cost Breakdown</h4>
    ${
      providerEntries.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Provider</th>
                <th>Requests</th>
                <th>Tokens</th>
                <th>Cost</th>
              </tr>
            </thead>
            <tbody>
              ${providerEntries
                .map(
                  ([provider, summary]) => `
                    <tr>
                      <td>${escapeHtml(provider)}</td>
                      <td>${formatInteger(summary.request_count)} / ${formatInteger(summary.response_count)}</td>
                      <td>${formatInteger(summary.total_tokens)}</td>
                      <td>${formatCents(summary.estimated_cost_cents)}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No provider usage yet.</div>`
    }
    <h4>Tool Runtime Breakdown</h4>
    ${
      toolEntries.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Tool</th>
                <th>Calls</th>
                <th>Status</th>
                <th>Runtime</th>
              </tr>
            </thead>
            <tbody>
              ${toolEntries
                .map(
                  ([tool, summary]) => `
                    <tr>
                      <td>${escapeHtml(tool)}</td>
                      <td>${formatInteger(summary.call_count)}</td>
                      <td>${formatInteger(summary.success_count)} ok · ${formatInteger(summary.failed_count)} failed</td>
                      <td>${formatDurationMs(summary.total_duration_ms)}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No tool runtime data yet.</div>`
    }
  `;
  usageRollupRoot.innerHTML = state.usageRollups.length
    ? state.usageRollups
        .map(
          (rollup) => `
            <div class="item">
              <strong>${escapeHtml(formatCents(rollup.summary.estimated_provider_cost_cents))}</strong>
              <div class="muted">${escapeHtml(rollup.period_start)} to ${escapeHtml(rollup.period_end)}</div>
              <div class="muted">${escapeHtml(formatInteger(rollup.summary.total_tokens))} provider tokens · ${escapeHtml(formatInteger(rollup.summary.tool_call_count))} tool calls</div>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No persisted usage rollups</div>`;
}

function formatInteger(value) {
  return Number(value || 0).toLocaleString("en-US", { maximumFractionDigits: 0 });
}

function formatCents(value) {
  return `${Number(value || 0).toFixed(2)} cents`;
}

function formatDurationMs(value) {
  const milliseconds = Number(value || 0);
  if (milliseconds >= 1000) {
    return `${(milliseconds / 1000).toFixed(2)}s`;
  }
  return `${milliseconds.toFixed(0)}ms`;
}

function renderProviders() {
  providerRoot.innerHTML = state.providers.length
    ? state.providers
        .map(
          (provider) => `
            <div class="item">
              <strong>${escapeHtml(provider.name)}</strong>
              <div class="muted">${escapeHtml(provider.provider_type)} · ${escapeHtml(provider.status)}</div>
              <button class="secondary" data-provider-status="${provider.id}" data-status="active">Activate</button>
              <button class="secondary reject" data-provider-status="${provider.id}" data-status="disabled">Disable</button>
              <pre>${escapeHtml(JSON.stringify(provider.config, null, 2))}</pre>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No stored providers</div>`;
  providerRoot.querySelectorAll("[data-provider-status]").forEach((button) => {
    button.addEventListener("click", () =>
      updateProviderStatus(button.dataset.providerStatus, button.dataset.status),
    );
  });
}

async function updateProviderStatus(providerId, status) {
  await api(`/api/providers/${providerId}/status`, {
    method: "PATCH",
    body: JSON.stringify({ status }),
  });
  await refreshOps();
}

function renderEvalRuns() {
  evalRunRoot.innerHTML = state.evalRuns.length
    ? state.evalRuns
        .map(
          (run) => {
            const gate = state.evalGates[run.id];
            return `
            <div class="item">
              <strong>${escapeHtml(run.status)} · score ${escapeHtml(run.score ?? "n/a")}</strong>
              <div class="muted">${escapeHtml(run.created_at)}</div>
              <button class="secondary" data-eval-gate="${run.id}">Gate 100%</button>
              ${
                gate
                  ? `<div class="muted">Gate: ${escapeHtml(gate.status)} · min ${escapeHtml(gate.min_score)}</div>
                     <pre>${escapeHtml(JSON.stringify(gate.failure_reasons, null, 2))}</pre>`
                  : ""
              }
              <pre>${escapeHtml(JSON.stringify(run.details, null, 2))}</pre>
            </div>
          `;
          },
        )
        .join("")
    : `<div class="muted">No eval runs</div>`;
  evalRunRoot.querySelectorAll("[data-eval-gate]").forEach((button) => {
    button.addEventListener("click", () => gateEvalRun(button.dataset.evalGate));
  });
}

function renderEvalDatasets() {
  evalDatasetRoot.innerHTML = state.evalDatasets.length
    ? state.evalDatasets
        .map(
          (dataset) => `
            <button class="item-button" data-eval-dataset="${dataset.id}">
              <strong>${escapeHtml(dataset.name)}</strong>
              <span>${escapeHtml(dataset.id)}</span>
            </button>
          `,
        )
        .join("")
    : `<div class="muted">No eval datasets</div>`;
  evalDatasetRoot.querySelectorAll("[data-eval-dataset]").forEach((button) => {
    button.addEventListener("click", () => {
      setEvalDatasetId(button.dataset.evalDataset);
      loadEvalCases();
    });
  });
}

function renderEvalCases() {
  evalCaseRoot.innerHTML = state.evalCases.length
    ? state.evalCases
        .map(
          (testCase) => `
            <div class="item">
              <strong>${escapeHtml(testCase.grading_policy.kind || "eval case")}</strong>
              <div class="muted">${escapeHtml(testCase.id)}</div>
              <pre>${escapeHtml(JSON.stringify({ input: testCase.input, expected: testCase.expected }, null, 2))}</pre>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No loaded eval cases</div>`;
}

function setEvalDatasetId(datasetId) {
  evalCaseForm.elements.dataset_id.value = datasetId;
  evalRunForm.elements.dataset_id.value = datasetId;
}

function setOrganizationId(organizationId) {
  state.selectedOrganizationId = organizationId;
  teamForm.elements.organization_id.value = organizationId;
  membershipForm.elements.organization_id.value = organizationId;
}

function setTeamId(teamId) {
  state.selectedTeamId = teamId;
  projectForm.elements.team_id.value = teamId;
  membershipForm.elements.team_id.value = teamId;
  mcpForm.elements.team_id.value = teamId;
}

function parseJsonField(value, label) {
  try {
    return JSON.parse(String(value || "{}"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function renderMcpServers() {
  mcpServerRoot.innerHTML = state.mcpServers.length
    ? state.mcpServers
        .map(
          (server) => `
            <div class="item">
              <strong>${escapeHtml(server.name)}</strong>
              <div class="muted">${escapeHtml(server.transport)} · ${escapeHtml(server.status)}</div>
              <div class="muted">Tools: ${escapeHtml(server.tool_allowlist.join(", ") || "none")}</div>
              <button class="secondary" data-discover-mcp="${server.id}">Discover Tools</button>
              <pre>${escapeHtml(JSON.stringify(server.config, null, 2))}</pre>
            </div>
          `,
        )
        .join("")
    : state.mcpTeamId
      ? `<div class="muted">No MCP servers for this team</div>`
      : `<div class="muted">Enter a team ID to manage MCP servers</div>`;
  mcpServerRoot.querySelectorAll("[data-discover-mcp]").forEach((button) => {
    button.addEventListener("click", () => discoverMcpTools(button.dataset.discoverMcp));
  });
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
  const originalArgs = approval.evidence?.args ?? {};
  const modifiedArgs = approval.decision_payload?.modified_args;
  return `
    <div class="item">
      <strong>${escapeHtml(approval.action)}</strong>
      <div class="muted">${escapeHtml(approval.risk_level)} · ${escapeHtml(approval.status)}</div>
      <div class="muted">Expires: ${escapeHtml(approval.expires_at || "not set")}</div>
      <p>${escapeHtml(approval.reason)}</p>
      <dl>
        <dt>Original args</dt>
        <dd><pre>${escapeHtml(JSON.stringify(originalArgs, null, 2))}</pre></dd>
        ${
          modifiedArgs
            ? `<dt>Modified args</dt><dd><pre>${escapeHtml(JSON.stringify(modifiedArgs, null, 2))}</pre></dd>`
            : ""
        }
        <dt>Decision payload</dt>
        <dd><pre>${escapeHtml(JSON.stringify(approval.decision_payload ?? {}, null, 2))}</pre></dd>
      </dl>
      ${
        isPending
          ? `<form class="stack-form compact-form" data-approval-modify="${approval.id}">
              <label>
                Args JSON
                <textarea name="args" rows="4">${escapeHtml(JSON.stringify(modifiedArgs ?? originalArgs, null, 2))}</textarea>
              </label>
              <label>
                Comment
                <input name="comment" value="${escapeHtml(approval.decision_payload?.comment ?? "")}" />
              </label>
              <button type="submit" class="secondary">Modify Args</button>
            </form>
            <button class="secondary" data-approve="${approval.id}">Approve</button><button class="secondary reject" data-reject="${approval.id}">Reject</button><button class="secondary" data-expire="${approval.id}">Expire</button>`
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
