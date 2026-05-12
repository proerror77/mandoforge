const state = {
  agents: [],
  session: null,
  events: [],
  artifacts: [],
  toolCalls: [],
  auditLogs: [],
  providers: [],
  providerHealth: {},
  vaultHealth: null,
  secretRecords: [],
  policy: null,
  policyDecision: null,
  policyRuntime: null,
  policyTest: null,
  policyRevisions: [],
  policyRevisionDiffs: {},
  policyRevisionGates: {},
  evalDatasets: [],
  evalCases: [],
  evalRuns: [],
  evalGates: {},
  evalDrifts: {},
  agentReleases: {},
  mcpServers: [],
  mcpTeamId: "",
  mcpHealth: {},
  mcpHealthRun: null,
  mcpScheduledHealthRun: null,
  executionJobs: [],
  usageRollups: [],
  costAlertRoutes: [],
  usage: null,
  costAlertDelivery: null,
  costAlertAcknowledgement: null,
  organizations: [],
  teams: [],
  projects: [],
  memberships: [],
  selectedOrganizationId: "",
  selectedTeamId: "",
  approvalDeliveries: {},
  approvalGroups: [],
  approvalEscalationRules: [],
  codexAppServer: {
    health: null,
    thread: null,
    turn: null,
    command: null,
    interrupt: null,
    sync: null,
    runs: [],
    error: null,
  },
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
const vaultHealthRoot = document.querySelector("#vault-health");
const secretRecordRoot = document.querySelector("#secret-records");
const checkVaultHealthButton = document.querySelector("#check-vault-health");
const policyRoot = document.querySelector("#policy-summary");
const policyForm = document.querySelector("#policy-simulate-form");
const policyTestForm = document.querySelector("#policy-test-form");
const policyDecisionRoot = document.querySelector("#policy-decision");
const policyRevisionForm = document.querySelector("#policy-revision-form");
const policyRevisionRoot = document.querySelector("#policy-revisions");
const policyGateCasesInput = document.querySelector("#policy-gate-cases");
const policyRolloutPercentInput = document.querySelector("#policy-rollout-percent");
const policyActivateAfterInput = document.querySelector("#policy-activate-after");
const policyActivateBeforeInput = document.querySelector("#policy-activate-before");
const cancelPolicyRolloutButton = document.querySelector("#cancel-policy-rollout");
const evalDatasetRoot = document.querySelector("#eval-datasets");
const evalCaseRoot = document.querySelector("#eval-cases");
const evalRunRoot = document.querySelector("#eval-runs");
const agentReleaseRoot = document.querySelector("#agent-releases");
const mcpServerRoot = document.querySelector("#mcp-servers");
const executionJobRoot = document.querySelector("#execution-jobs");
const usageRoot = document.querySelector("#usage-summary");
const usageRollupRoot = document.querySelector("#usage-rollups");
const costAlertRouteRoot = document.querySelector("#cost-alert-routes");
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
const approvalGroupForm = document.querySelector("#approval-group-form");
const approvalEscalationRuleForm = document.querySelector("#approval-escalation-rule-form");
const approvalGovernanceRoot = document.querySelector("#approval-governance");
const providerForm = document.querySelector("#provider-form");
const secretForm = document.querySelector("#secret-form");
const evalDatasetForm = document.querySelector("#eval-dataset-form");
const evalCaseForm = document.querySelector("#eval-case-form");
const evalRunForm = document.querySelector("#eval-run-form");
const mcpForm = document.querySelector("#mcp-form");
const loadMcpButton = document.querySelector("#load-mcp");
const runMcpHealthButton = document.querySelector("#run-mcp-health");
const runDueMcpHealthButton = document.querySelector("#run-due-mcp-health");
const loadEvalCasesButton = document.querySelector("#load-eval-cases");
const refreshExecutionJobsButton = document.querySelector("#refresh-execution-jobs");
const createUsageRollupButton = document.querySelector("#create-usage-rollup");
const deliverCostAlertsButton = document.querySelector("#deliver-cost-alerts");
const costAlertRouteForm = document.querySelector("#cost-alert-route-form");
const checkCodexHealthButton = document.querySelector("#check-codex-health");
const loadCodexRunsButton = document.querySelector("#load-codex-runs");
const codexThreadForm = document.querySelector("#codex-thread-form");
const codexTurnForm = document.querySelector("#codex-turn-form");
const codexCommandForm = document.querySelector("#codex-command-form");
const codexArtifactSyncForm = document.querySelector("#codex-artifact-sync-form");
const interruptCodexTurnButton = document.querySelector("#interrupt-codex-turn");
const codexAppServerRoot = document.querySelector("#codex-app-server");

document.querySelector("#new-session").addEventListener("click", runDemo);
agentForm.addEventListener("submit", createAgent);
organizationForm.addEventListener("submit", createOrganization);
teamForm.addEventListener("submit", createTeam);
projectForm.addEventListener("submit", createProject);
membershipForm.addEventListener("submit", createMembership);
approvalGroupForm.addEventListener("submit", createApprovalGroup);
approvalEscalationRuleForm.addEventListener("submit", createApprovalEscalationRule);
providerForm.addEventListener("submit", createProvider);
secretForm.addEventListener("submit", createSecretRecord);
checkVaultHealthButton.addEventListener("click", checkVaultHealth);
policyForm.addEventListener("submit", simulatePolicy);
policyTestForm.addEventListener("submit", testPolicy);
policyRevisionForm.addEventListener("submit", createPolicyRevision);
cancelPolicyRolloutButton.addEventListener("click", cancelPolicyRollout);
evalDatasetForm.addEventListener("submit", createEvalDataset);
evalCaseForm.addEventListener("submit", createEvalCase);
evalRunForm.addEventListener("submit", createEvalRun);
mcpForm.addEventListener("submit", createMcpServer);
loadMcpButton.addEventListener("click", loadMcpServers);
runMcpHealthButton.addEventListener("click", runMcpHealth);
runDueMcpHealthButton.addEventListener("click", runDueMcpHealth);
loadEvalCasesButton.addEventListener("click", loadEvalCases);
refreshExecutionJobsButton.addEventListener("click", refreshExecutionJobs);
createUsageRollupButton.addEventListener("click", createUsageRollup);
deliverCostAlertsButton.addEventListener("click", deliverCostAlerts);
costAlertRouteForm.addEventListener("submit", createCostAlertRoute);
checkCodexHealthButton.addEventListener("click", checkCodexAppServerHealth);
loadCodexRunsButton.addEventListener("click", loadCodexAppServerRuns);
codexThreadForm.addEventListener("submit", createCodexThread);
codexTurnForm.addEventListener("submit", createCodexTurn);
codexCommandForm.addEventListener("submit", executeCodexCommand);
codexArtifactSyncForm.addEventListener("submit", syncCodexArtifacts);
interruptCodexTurnButton.addEventListener("click", interruptCodexTurn);

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

async function createApprovalGroup(event) {
  event.preventDefault();
  const form = new FormData(approvalGroupForm);
  await api("/api/approval-groups", {
    method: "POST",
    body: JSON.stringify({
      name: String(form.get("name") || "").trim(),
      subjects: String(form.get("subjects") || "")
        .split(/[,\n]/)
        .map((subject) => subject.trim())
        .filter(Boolean),
    }),
  });
  await refreshOps();
}

async function createApprovalEscalationRule(event) {
  event.preventDefault();
  const form = new FormData(approvalEscalationRuleForm);
  await api("/api/approval-escalation-rules", {
    method: "POST",
    body: JSON.stringify({
      name: String(form.get("name") || "").trim(),
      risk_level: String(form.get("risk_level") || "").trim(),
      group_id: String(form.get("group_id") || "").trim(),
      order_index: 0,
      after_seconds: 0,
    }),
  });
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
  const baseUrl = String(form.get("base_url") || "").trim();
  const apiKeyEnv = String(form.get("api_key_env") || "").trim();
  const apiKeyRef = String(form.get("api_key_ref") || "").trim();
  const config = {
    budget: {
      daily_request_limit: dailyRequestLimit,
      daily_cost_limit_cents: dailyCostLimitCents,
    },
    pricing: {
      per_request_cents: perRequestCents,
      per_1k_prompt_tokens_cents: promptTokenCents,
      per_1k_completion_tokens_cents: completionTokenCents,
    },
  };
  if (apiKeyEnv) config.api_key_env = apiKeyEnv;
  if (apiKeyRef) config.api_key_ref = apiKeyRef;
  await api("/api/providers", {
    method: "POST",
    body: JSON.stringify({
      name: form.get("name"),
      provider_type: form.get("provider_type"),
      base_url: baseUrl || null,
      default_model: form.get("default_model"),
      config,
    }),
  });
  await refreshOps();
}

async function createSecretRecord(event) {
  event.preventDefault();
  const form = new FormData(secretForm);
  const scopeId = String(form.get("scope_id") || "").trim();
  const payload = {
    name: String(form.get("name") || "").trim(),
    path: String(form.get("path") || "").trim(),
    key: String(form.get("key") || "").trim(),
    scope_type: String(form.get("scope_type") || "tenant").trim(),
  };
  if (scopeId) payload.scope_id = scopeId;
  await api("/api/vault/secrets", {
    method: "POST",
    body: JSON.stringify(payload),
  });
  await refreshOps();
}

async function rotateSecretRecord(id, path, key) {
  await api(`/api/vault/secrets/${id}/rotate`, {
    method: "POST",
    body: JSON.stringify({ path, key }),
  });
  await refreshOps();
}

async function simulatePolicy(event) {
  event.preventDefault();
  const form = new FormData(event.currentTarget);
  state.policyDecision = await api("/api/policy/simulate", {
    method: "POST",
    body: JSON.stringify({
      tool_name: String(form.get("tool_name") || "").trim(),
    }),
  });
  renderPolicy();
}

async function testPolicy(event) {
  event.preventDefault();
  const form = new FormData(event.currentTarget);
  const toolNames = String(form.get("tool_names") || "")
    .split(/[,\n]/)
    .map((tool) => tool.trim())
    .filter(Boolean);
  state.policyTest = await api("/api/policy/test", {
    method: "POST",
    body: JSON.stringify({ tool_names: toolNames }),
  });
  renderPolicy();
}

async function createPolicyRevision(event) {
  event.preventDefault();
  const form = new FormData(policyRevisionForm);
  await api("/api/policy/revisions", {
    method: "POST",
    body: JSON.stringify({
      name: String(form.get("name") || "").trim(),
      body: parseJsonField(form.get("body"), "Policy body JSON"),
    }),
  });
  state.policyRevisions = await api("/api/policy/revisions");
  renderPolicy();
}

async function activatePolicyRevision(id) {
  await api(`/api/policy/revisions/${id}/activate`, { method: "POST" });
  state.policyRuntime = await api("/api/policy/runtime");
  state.policyRevisions = await api("/api/policy/revisions");
  renderPolicy();
}

async function cancelPolicyRollout() {
  state.policyRuntime = await api("/api/policy/rollout/cancel", { method: "POST" });
  renderPolicy();
}

async function diffPolicyRevision(id) {
  state.policyRevisionDiffs[id] = await api(`/api/policy/revisions/${id}/diff`);
  renderPolicy();
}

async function gatePolicyRevision(id) {
  state.policyRevisionGates[id] = await api(`/api/policy/revisions/${id}/gate`, {
    method: "POST",
    body: JSON.stringify({
      cases: parseJsonField(policyGateCasesInput.value, "Gate cases JSON"),
      rollout_percent: Number(policyRolloutPercentInput.value || 100),
      activate_after: policyActivateAfterInput.value.trim() || null,
      activate_before: policyActivateBeforeInput.value.trim() || null,
    }),
  });
  state.policyRevisions = await api("/api/policy/revisions");
  renderPolicy();
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

async function driftEvalRun(runId) {
  const decision = await api(`/api/eval/runs/${runId}/drift`);
  state.evalDrifts[runId] = decision;
  renderEvalRuns();
}

async function promoteEvalRun(runId, environment) {
  const run = state.evalRuns.find((candidate) => candidate.id === runId);
  if (!run) return;
  await api(`/api/agents/${run.agent_id}/releases`, {
    method: "POST",
    body: JSON.stringify({
      eval_run_id: run.id,
      agent_version_id: run.agent_version_id,
      environment,
      min_score: 1.0,
    }),
  });
  await refreshAgentReleases();
}

async function rollbackAgentRelease(agentId, releaseId) {
  await api(`/api/agents/${agentId}/releases/${releaseId}/rollback`, {
    method: "POST",
  });
  await refreshAgentReleases();
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
      config: parseJsonField(form.get("config"), "MCP config"),
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

async function deliverCostAlerts() {
  state.costAlertDelivery = await api("/api/usage/alerts/deliver", {
    method: "POST",
  });
  await refreshOps();
}

async function createCostAlertRoute(event) {
  event.preventDefault();
  const form = new FormData(costAlertRouteForm);
  await api("/api/usage/alert-routes", {
    method: "POST",
    body: JSON.stringify({
      name: String(form.get("name") || "").trim(),
      channel: String(form.get("channel") || "").trim(),
      target: String(form.get("target") || "").trim() || null,
      severity_filter: String(form.get("severity_filter") || "").trim(),
    }),
  });
  await refreshOps();
}

async function checkCodexAppServerHealth() {
  await captureCodexAppServer("health", async () => {
    state.codexAppServer.health = await api("/api/codex-app-server/health");
  });
}

async function loadCodexAppServerRuns() {
  await captureCodexAppServer("runs", async () => {
    state.codexAppServer.runs = await api("/api/codex-app-server/runs");
  });
}

async function pollCodexRun(runId) {
  await captureCodexAppServer("poll", async () => {
    state.codexAppServer.poll = await api(`/api/codex-app-server/runs/${encodeURIComponent(runId)}/poll`, {
      method: "POST",
      body: JSON.stringify({
        max_attempts: 3,
        retry_interval_ms: 0,
      }),
    });
    state.codexAppServer.runs = await api("/api/codex-app-server/runs");
  });
}

async function createCodexThread(event) {
  event.preventDefault();
  const form = new FormData(codexThreadForm);
  await captureCodexAppServer("thread", async () => {
    const thread = await api("/api/codex-app-server/threads", {
      method: "POST",
      body: JSON.stringify({
        metadata: parseJsonField(form.get("metadata"), "Thread metadata JSON"),
      }),
    });
    state.codexAppServer.thread = thread;
    setCodexThreadId(thread.thread_id);
  });
}

async function createCodexTurn(event) {
  event.preventDefault();
  const form = new FormData(codexTurnForm);
  const threadId = String(form.get("thread_id") || "").trim();
  await captureCodexAppServer("turn", async () => {
    const turn = await api(`/api/codex-app-server/threads/${encodeURIComponent(threadId)}/turns`, {
      method: "POST",
      body: JSON.stringify({
        message: String(form.get("message") || "").trim(),
        metadata: parseJsonField(form.get("metadata"), "Turn metadata JSON"),
      }),
    });
    state.codexAppServer.turn = turn;
    setCodexTurnId(turn.turn_id);
    populateCodexArtifactSyncFromResponse(turn);
  });
}

async function executeCodexCommand(event) {
  event.preventDefault();
  const form = new FormData(codexCommandForm);
  const turnId = String(form.get("turn_id") || "").trim();
  await captureCodexAppServer("command", async () => {
    state.codexAppServer.command = await api(
      `/api/codex-app-server/turns/${encodeURIComponent(turnId)}/commands`,
      {
        method: "POST",
        body: JSON.stringify({
          command: String(form.get("command") || "").trim(),
          args: parseJsonField(form.get("args"), "Args JSON"),
        }),
      },
    );
    populateCodexArtifactSyncFromResponse(state.codexAppServer.command);
  });
}

async function interruptCodexTurn() {
  const form = new FormData(codexCommandForm);
  const turnId = String(form.get("turn_id") || "").trim();
  await captureCodexAppServer("interrupt", async () => {
    state.codexAppServer.interrupt = await api(
      `/api/codex-app-server/turns/${encodeURIComponent(turnId)}/interrupt`,
      { method: "POST" },
    );
  });
}

async function captureCodexAppServer(operation, action) {
  state.codexAppServer.error = null;
  try {
    await action();
  } catch (error) {
    state.codexAppServer.error = {
      operation,
      message: error.message,
    };
  }
  renderCodexAppServer();
}

async function syncCodexArtifacts(event) {
  event.preventDefault();
  const form = new FormData(codexArtifactSyncForm);
  await captureCodexAppServer("sync", async () => {
    const sessionId = String(form.get("session_id") || state.session?.id || "").trim();
    const artifacts = parseJsonField(form.get("artifacts"), "Synced artifacts JSON");
    if (!Array.isArray(artifacts)) {
      throw new Error("Synced artifacts JSON must be an array");
    }
    state.codexAppServer.sync = await api("/api/codex-app-server/artifacts/sync", {
      method: "POST",
      body: JSON.stringify({
        session_id: sessionId,
        turn_id: state.codexAppServer.turn?.turn_id || null,
        command_id: state.codexAppServer.command?.command_id || null,
        artifacts,
      }),
    });
    if (state.session?.id === sessionId) {
      state.selectedArtifactId = null;
      await refreshSession();
    }
  });
}

async function acknowledgeCostAlert(providerName, severity) {
  state.costAlertAcknowledgement = await api("/api/usage/alerts/ack", {
    method: "POST",
    body: JSON.stringify({
      provider_name: providerName,
      severity,
      comment: "Acknowledged from static Usage panel",
    }),
  });
  renderUsage();
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

async function checkMcpHealth(serverId) {
  if (!state.mcpTeamId) return;
  state.mcpHealth[serverId] = await api(
    `/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}/health`,
  );
  renderMcpServers();
}

async function runMcpHealth() {
  if (!state.mcpTeamId) {
    const form = new FormData(mcpForm);
    state.mcpTeamId = String(form.get("team_id") || "").trim();
  }
  if (!state.mcpTeamId) return;
  const run = await api(`/api/teams/${state.mcpTeamId}/mcp-servers/health/run`, {
    method: "POST",
  });
  state.mcpHealthRun = run;
  state.mcpHealth = Object.fromEntries((run.results || []).map((health) => [health.server_id, health]));
  renderMcpServers();
}

async function runDueMcpHealth() {
  if (!state.mcpTeamId) {
    const form = new FormData(mcpForm);
    state.mcpTeamId = String(form.get("team_id") || "").trim();
  }
  if (!state.mcpTeamId) return;
  const run = await api(`/api/teams/${state.mcpTeamId}/mcp-servers/health/run-due`, {
    method: "POST",
  });
  state.mcpScheduledHealthRun = run;
  state.mcpHealth = Object.fromEntries((run.results || []).map((health) => [health.server_id, health]));
  await loadMcpServers();
}

async function updateMcpStatus(serverId, status) {
  if (!state.mcpTeamId) return;
  await api(`/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}/status`, {
    method: "PATCH",
    body: JSON.stringify({ status }),
  });
  await loadMcpServers();
}

async function editMcpServer(serverId) {
  if (!state.mcpTeamId) return;
  const server = state.mcpServers.find((item) => item.id === serverId);
  if (!server) return;
  const transport = window.prompt("Transport", server.transport);
  if (transport === null) return;
  const tools = window.prompt("Tool allowlist", server.tool_allowlist.join(","));
  if (tools === null) return;
  const config = window.prompt("Config JSON", JSON.stringify(server.config, null, 2));
  if (config === null) return;
  await api(`/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}`, {
    method: "PATCH",
    body: JSON.stringify({
      transport,
      config: parseJsonField(config, "MCP config"),
      tool_allowlist: tools
        .split(",")
        .map((tool) => tool.trim())
        .filter(Boolean),
    }),
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
  setCodexSyncSessionId(state.session.id);
  state.selectedArtifactId = null;
  state.selectedToolCallId = null;
  state.selectedAuditLogId = null;
  await refreshSession();
  await refreshApprovals();
  await refreshOps();
}

async function refreshOps() {
  const [
    providers,
    secretRecords,
    policy,
    policyRuntime,
    policyRevisions,
    evalDatasets,
    evalRuns,
    usage,
    usageRollups,
    costAlertRoutes,
    organizations,
    executionJobs,
    approvalGroups,
    approvalEscalationRules,
  ] =
    await Promise.all([
      api("/api/providers"),
      api("/api/vault/secrets"),
      api("/api/policy"),
      api("/api/policy/runtime"),
      api("/api/policy/revisions"),
      api("/api/eval/datasets"),
      api("/api/eval/runs"),
      api("/api/usage"),
      api("/api/usage/rollups"),
      api("/api/usage/alert-routes"),
      api("/api/organizations"),
      api("/api/execution-jobs"),
      api("/api/approval-groups"),
      api("/api/approval-escalation-rules"),
    ]);
  state.providers = providers;
  state.secretRecords = secretRecords;
  state.policy = policy;
  state.policyRuntime = policyRuntime;
  state.policyRevisions = policyRevisions;
  state.evalDatasets = evalDatasets;
  state.evalRuns = evalRuns;
  state.usage = usage;
  state.usageRollups = usageRollups;
  state.costAlertRoutes = costAlertRoutes;
  state.organizations = organizations;
  state.executionJobs = executionJobs;
  state.approvalGroups = approvalGroups;
  state.approvalEscalationRules = approvalEscalationRules;
  await refreshAgentReleases(false);
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

async function refreshAgentReleases(render = true) {
  const releaseEntries = await Promise.all(
    state.agents.map(async (agent) => [agent.id, await api(`/api/agents/${agent.id}/releases`)]),
  );
  state.agentReleases = Object.fromEntries(releaseEntries);
  if (render) {
    renderAgentReleases();
  }
}

async function refreshSession() {
  if (!state.session) return;
  state.session = await api(`/api/sessions/${state.session.id}`);
  setCodexSyncSessionId(state.session.id);
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
  approvalRoot.querySelectorAll("[data-deliver-approval]").forEach((button) => {
    button.addEventListener("click", () =>
      deliverApprovalNotification(button.dataset.deliverApproval),
    );
  });
  approvalRoot.querySelectorAll("[data-escalate-approval]").forEach((button) => {
    button.addEventListener("click", () => escalateApproval(button.dataset.escalateApproval));
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

async function deliverApprovalNotification(id) {
  state.approvalDeliveries[id] = await api(`/api/approvals/${id}/deliver`, {
    method: "POST",
  });
  await refreshApprovals();
}

async function escalateApproval(id) {
  await api(`/api/approvals/${id}/escalate`, {
    method: "POST",
    body: JSON.stringify({ reason: "Escalated from static console" }),
  });
  await refreshApprovals();
  await refreshSession();
  await refreshOps();
}

function renderOps() {
  renderUsage();
  renderTenantGovernance();
  renderProviders();
  renderVaultHealth();
  renderSecretRecords();
  renderApprovalGovernance();
  renderPolicy();
  renderEvalDatasets();
  renderEvalCases();
  renderEvalRuns();
  renderAgentReleases();
  renderMcpServers();
  renderExecutionJobs();
  renderCodexAppServer();
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

function renderCodexAppServer() {
  const codex = state.codexAppServer;
  const responseCards = [
    ["Health", codex.health],
    ["Thread", codex.thread],
    ["Turn", codex.turn],
    ["Command", codex.command],
    ["Interrupt", codex.interrupt],
    ["Poll", codex.poll],
    ["Sync", codex.sync],
  ]
    .filter(([, value]) => value)
    .map(
      ([label, value]) => `
        <div class="item">
          <strong>${escapeHtml(label)}</strong>
          <pre>${escapeHtml(JSON.stringify(value, null, 2))}</pre>
        </div>
      `,
    )
    .join("");
  const runs = (codex.runs || [])
    .slice(0, 10)
    .map(
      (run) => `
        <div class="item">
          <strong>${escapeHtml(run.operation)}</strong>
          <div class="muted">${escapeHtml(run.status)} · ${escapeHtml(run.thread_id || "no thread")} · ${escapeHtml(run.turn_id || "no turn")} · ${escapeHtml(run.created_at)}</div>
          ${
            run.turn_id
              ? `<button type="button" class="secondary" data-poll-codex-run="${escapeHtml(run.id)}">Poll Run</button>`
              : ""
          }
          <pre>${escapeHtml(JSON.stringify(run.response, null, 2))}</pre>
        </div>
      `,
    )
    .join("");
  codexAppServerRoot.innerHTML = `
    <div class="item">
      <strong>Codex steering</strong>
      <div class="muted">Routes stay fail-closed unless MANDOFORGE_CODEX_APP_SERVER_URL is configured; steering responses are persisted for replay, and synced artifacts are imported into session artifacts, timeline, and audit.</div>
    </div>
    ${
      codex.error
        ? `<div class="item danger">
            <strong>${escapeHtml(codex.error.operation)} failed</strong>
            <pre>${escapeHtml(codex.error.message)}</pre>
          </div>`
        : ""
    }
    ${responseCards || `<div class="muted">No Codex App Server responses yet.</div>`}
    ${runs ? `<h4>Persisted Codex Runs</h4>${runs}` : ""}
  `;
  codexAppServerRoot.querySelectorAll("[data-poll-codex-run]").forEach((button) => {
    button.addEventListener("click", () => pollCodexRun(button.dataset.pollCodexRun));
  });
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
              <div class="muted">Attempts: ${formatInteger(job.attempt_count || 0)}/${formatInteger(job.max_attempts || 0)} · Last error: ${escapeHtml(job.last_error || "none")}</div>
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
                    attempt_count: job.attempt_count,
                    max_attempts: job.max_attempts,
                    last_error: job.last_error,
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
  const budgetEntries = usage.provider_budgets || [];
  const costAlertDelivery = state.costAlertDelivery;
  const costAlertAcknowledgement = state.costAlertAcknowledgement;
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
    <h4>Provider Budget Forecast</h4>
    ${
      costAlertDelivery
        ? `<div class="item">
            <strong>Cost alert delivery: ${escapeHtml(costAlertDelivery.status)}</strong>
            <div class="muted">${escapeHtml(costAlertDelivery.channel)} · ${escapeHtml(costAlertDelivery.webhook_configured ? "webhook configured" : "webhook not configured")} · ${formatInteger((costAlertDelivery.alerts || []).length)} alerts</div>
            <pre>${escapeHtml(JSON.stringify(costAlertDelivery.route_deliveries || [], null, 2))}</pre>
          </div>`
        : ""
    }
    ${
      costAlertAcknowledgement
        ? `<div class="item">
            <strong>Cost alert acknowledged: ${escapeHtml(costAlertAcknowledgement.provider_name)}</strong>
            <div class="muted">${escapeHtml(costAlertAcknowledgement.severity)} · ${escapeHtml(costAlertAcknowledgement.acknowledged_by)} · ${escapeHtml(costAlertAcknowledgement.acknowledged_at)}</div>
          </div>`
        : ""
    }
    ${
      budgetEntries.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Provider</th>
                <th>Status</th>
                <th>Requests</th>
                <th>Cost</th>
                <th>Messages</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              ${budgetEntries
                .map(
                  (budget) => `
                    <tr>
                      <td>${escapeHtml(budget.provider_name)}</td>
                      <td><span class="budget-status ${escapeHtml(budget.status)}">${escapeHtml(budget.status)}</span></td>
                      <td>${formatInteger(budget.request_count)} / ${formatOptionalInteger(budget.daily_request_limit)} · ${formatOptionalPercent(budget.request_budget_used_percent)}</td>
                      <td>${formatCents(budget.estimated_cost_cents)} / ${formatOptionalCents(budget.daily_cost_limit_cents)} · ${formatOptionalPercent(budget.cost_budget_used_percent)}</td>
                      <td>${escapeHtml((budget.messages || []).join(" | ") || "No budget pressure")}</td>
                      <td>${
                        budget.status === "warning" || budget.status === "critical"
                          ? `<button class="secondary" data-cost-alert-ack="${escapeHtml(budget.provider_name)}" data-cost-alert-severity="${escapeHtml(budget.status)}">Ack</button>`
                          : `<span class="muted">No alert</span>`
                      }</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No provider budgets configured.</div>`
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
  usageRoot.querySelectorAll("[data-cost-alert-ack]").forEach((button) => {
    button.addEventListener("click", () =>
      acknowledgeCostAlert(button.dataset.costAlertAck, button.dataset.costAlertSeverity),
    );
  });
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
  costAlertRouteRoot.innerHTML = state.costAlertRoutes.length
    ? state.costAlertRoutes
        .map(
          (route) => `
            <div class="item">
              <strong>${escapeHtml(route.name)}</strong>
              <div class="muted">${escapeHtml(route.channel)} · ${escapeHtml(route.severity_filter)} · ${escapeHtml(route.status)}</div>
              <div class="muted">${escapeHtml(route.target || "reserved target")}</div>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No cost alert routes</div>`;
}

function formatInteger(value) {
  return Number(value || 0).toLocaleString("en-US", { maximumFractionDigits: 0 });
}

function formatCents(value) {
  return `${Number(value || 0).toFixed(2)} cents`;
}

function formatOptionalCents(value) {
  return value === null || value === undefined ? "none" : formatCents(value);
}

function formatOptionalInteger(value) {
  return value === null || value === undefined ? "none" : formatInteger(value);
}

function formatOptionalPercent(value) {
  return value === null || value === undefined ? "n/a" : `${Number(value || 0).toFixed(1)}%`;
}

function formatDurationMs(value) {
  const milliseconds = Number(value || 0);
  if (milliseconds >= 1000) {
    return `${(milliseconds / 1000).toFixed(2)}s`;
  }
  return `${milliseconds.toFixed(0)}ms`;
}

function renderPolicy() {
  if (!state.policy) {
    policyRoot.innerHTML = `<div class="muted">Policy data is not loaded.</div>`;
    policyDecisionRoot.innerHTML = "";
    policyRevisionRoot.innerHTML = "";
    return;
  }
  const blockedTools = state.policy.blocked_tools || [];
  const approvalRequired = state.policy.approval_required || [];
  const allowedTools = state.policy.allowed_tools || {};
  const runtime = state.policyRuntime || {};
  policyRoot.innerHTML = `
    <dl>
      <dt>Runtime rollout</dt>
      <dd>${escapeHtml(runtime.rollout_active ? `staged ${runtime.staged_rollout_percent}%` : "baseline only")}</dd>
      <dt>Staged revision</dt>
      <dd>${escapeHtml(runtime.staged_revision_id || "none")}</dd>
      <dt>Blocked tools</dt>
      <dd>${escapeHtml(blockedTools.join(", ") || "none")}</dd>
      <dt>Approval required</dt>
      <dd>${escapeHtml(approvalRequired.map((rule) => `${rule.tool}:${rule.risk}`).join(", ") || "none")}</dd>
      <dt>Allowed tool profiles</dt>
      <dd>${escapeHtml(Object.keys(allowedTools).join(", ") || "none")}</dd>
      <dt>SQL max rows</dt>
      <dd>${formatInteger(state.policy.sql_policy?.max_rows)}</dd>
    </dl>
  `;
  policyDecisionRoot.innerHTML = state.policyDecision
    ? `<div class="item">
        <strong>${escapeHtml(state.policyDecision.decision)}</strong>
        <div class="muted">${escapeHtml(state.policyDecision.risk_level)}</div>
        <pre>${escapeHtml(JSON.stringify(state.policyDecision, null, 2))}</pre>
      </div>`
    : `<div class="muted">No policy simulation yet.</div>`;
  if (state.policyTest) {
    policyDecisionRoot.innerHTML += `
      <div class="item">
        <strong>Policy test</strong>
        <div class="muted">${escapeHtml(state.policyTest.tested_at)}</div>
        <pre>${escapeHtml(JSON.stringify(state.policyTest.decisions, null, 2))}</pre>
      </div>
    `;
  }
  policyRevisionRoot.innerHTML = `
    <h4>Policy Revisions</h4>
    ${
      state.policyRevisions.length
        ? state.policyRevisions
            .map(
              (revision) => {
                const diff = state.policyRevisionDiffs[revision.id];
                const gate = state.policyRevisionGates[revision.id] || revision.gate_result;
                return `
                  <div class="item">
                    <strong>${escapeHtml(revision.name)}</strong>
                    <div class="muted">${escapeHtml(revision.status)} · gate ${escapeHtml(revision.gate_status || "not_run")} · rollout ${escapeHtml(gate?.rollout_percent ?? "n/a")}% · ${escapeHtml(policyActivationWindowLabel(gate))} · ${escapeHtml(revision.created_at)}</div>
                    <pre>${escapeHtml(JSON.stringify(revision.body, null, 2))}</pre>
                    <button type="button" data-policy-diff="${escapeHtml(revision.id)}">Diff</button>
                    <button type="button" data-policy-gate="${escapeHtml(revision.id)}">Gate</button>
                    ${
                      revision.status === "active"
                        ? `<span class="badge">Active</span>`
                        : revision.gate_status === "passed"
                          ? `<button type="button" data-policy-activate="${escapeHtml(revision.id)}">Activate</button>`
                          : `<span class="muted">Activation requires passed gate</span>`
                    }
                    ${
                      diff
                        ? renderPolicyDiffSummary(diff)
                        : ""
                    }
                    ${
                      gate && Object.keys(gate).length
                        ? renderPolicyGateSummary(gate, diff)
                        : ""
                    }
                  </div>
                `;
              },
            )
            .join("")
        : `<div class="muted">No policy revisions yet.</div>`
    }
  `;
  policyRevisionRoot.querySelectorAll("[data-policy-diff]").forEach((button) => {
    button.addEventListener("click", () => diffPolicyRevision(button.dataset.policyDiff));
  });
  policyRevisionRoot.querySelectorAll("[data-policy-gate]").forEach((button) => {
    button.addEventListener("click", () => gatePolicyRevision(button.dataset.policyGate));
  });
  policyRevisionRoot.querySelectorAll("[data-policy-activate]").forEach((button) => {
    button.addEventListener("click", () => activatePolicyRevision(button.dataset.policyActivate));
  });
  cancelPolicyRolloutButton.disabled = !state.policyRuntime?.rollout_active;
}

function policyActivationWindowLabel(gate) {
  const window = gate?.activation_window;
  if (!window) return "no activation window";
  const after = window.activate_after || "now";
  const before = window.activate_before || "open-ended";
  return `${after} to ${before}`;
}

function renderPolicyDiffSummary(diff) {
  const changes = diff?.changes || [];
  const counts = countPolicyDiffChanges(changes);
  const rows = changes.length
    ? changes
        .map(
          (change) => `
            <tr>
              <td><span class="diff-kind ${escapeHtml(change.kind)}">${escapeHtml(change.kind)}</span></td>
              <td><code>${escapeHtml(change.path || "$")}</code></td>
              <td><pre>${escapeHtml(formatDiffValue(change.current, false))}</pre></td>
              <td><pre>${escapeHtml(formatDiffValue(change.proposed, false))}</pre></td>
            </tr>
          `,
        )
        .join("")
    : `<tr><td colspan="4" class="muted">No policy changes detected.</td></tr>`;
  return `
    <div class="policy-diff-summary">
      <div class="metric-grid compact-metrics">
        <div class="metric"><span>Total Changes</span><strong>${formatInteger(changes.length)}</strong></div>
        <div class="metric"><span>Added</span><strong>${formatInteger(counts.added)}</strong></div>
        <div class="metric"><span>Changed</span><strong>${formatInteger(counts.changed)}</strong></div>
        <div class="metric"><span>Removed</span><strong>${formatInteger(counts.removed)}</strong></div>
      </div>
      <table class="diff-table policy-diff-table">
        <thead>
          <tr>
            <th>Change</th>
            <th>Path</th>
            <th>Current</th>
            <th>Proposed</th>
          </tr>
        </thead>
        <tbody>${rows}</tbody>
      </table>
      <div class="muted">Generated ${escapeHtml(diff.generated_at || "n/a")}</div>
    </div>
  `;
}

function renderPolicyGateSummary(gate, loadedDiff) {
  const cases = gate.cases || [];
  const passed = cases.filter((testCase) => testCase.passed).length;
  const failed = cases.length - passed;
  const diff = loadedDiff || gate.diff;
  const diffSummary = diff ? countPolicyDiffChanges(diff.changes || []) : null;
  const rows = cases.length
    ? cases
        .map(
          (testCase) => `
            <tr>
              <td><span class="budget-status ${testCase.passed ? "ok" : "critical"}">${escapeHtml(testCase.passed ? "passed" : "failed")}</span></td>
              <td><code>${escapeHtml(testCase.tool_name)}</code></td>
              <td>${escapeHtml(testCase.expected_decision)}</td>
              <td>${escapeHtml(testCase.actual_decision)}</td>
              <td>${escapeHtml(testCase.reason)}</td>
            </tr>
          `,
        )
        .join("")
    : `<tr><td colspan="5" class="muted">No gate cases recorded.</td></tr>`;
  return `
    <div class="policy-gate-summary">
      <div class="metric-grid compact-metrics">
        <div class="metric"><span>Gate Status</span><strong>${escapeHtml(gate.status || "unknown")}</strong></div>
        <div class="metric"><span>Rollout</span><strong>${escapeHtml(gate.rollout_percent === null || gate.rollout_percent === undefined ? "n/a" : `${gate.rollout_percent}%`)}</strong></div>
        <div class="metric"><span>Activation Window</span><strong>${escapeHtml(policyActivationWindowLabel(gate))}</strong></div>
        <div class="metric"><span>Cases Passed</span><strong>${formatInteger(passed)}/${formatInteger(cases.length)}</strong></div>
        <div class="metric"><span>Cases Failed</span><strong>${formatInteger(failed)}</strong></div>
      </div>
      ${
        diffSummary
          ? `<div class="muted">Diff summary: ${formatInteger((diff.changes || []).length)} changes · ${formatInteger(diffSummary.added)} added · ${formatInteger(diffSummary.changed)} changed · ${formatInteger(diffSummary.removed)} removed</div>`
          : ""
      }
      <table class="diff-table policy-gate-table">
        <thead>
          <tr>
            <th>Status</th>
            <th>Tool</th>
            <th>Expected</th>
            <th>Actual</th>
            <th>Reason</th>
          </tr>
        </thead>
        <tbody>${rows}</tbody>
      </table>
      <div class="muted">Suite ${escapeHtml(gate.suite_source || "n/a")} · checked ${escapeHtml(gate.checked_at || "n/a")}</div>
    </div>
  `;
}

function countPolicyDiffChanges(changes = []) {
  return changes.reduce(
    (counts, change) => {
      const kind = change.kind || "changed";
      counts[kind] = (counts[kind] || 0) + 1;
      return counts;
    },
    { added: 0, changed: 0, removed: 0 },
  );
}

function renderProviders() {
  providerRoot.innerHTML = state.providers.length
    ? state.providers
        .map(
          (provider) => {
            const health = state.providerHealth[provider.id];
            return `
            <div class="item">
              <strong>${escapeHtml(provider.name)}</strong>
              <div class="muted">${escapeHtml(provider.provider_type)} · ${escapeHtml(provider.status)}</div>
              <div class="muted">${escapeHtml(provider.default_model || "no default model")} · ${escapeHtml(provider.base_url || "no base URL")}</div>
              <button class="secondary" data-provider-status="${provider.id}" data-status="active">Activate</button>
              <button class="secondary reject" data-provider-status="${provider.id}" data-status="disabled">Disable</button>
              <button class="secondary" data-provider-health="${provider.id}">Check Health</button>
              ${
                health
                  ? `<div class="muted">Health: ${escapeHtml(health.healthy ? "healthy" : "unhealthy")} · ${escapeHtml(health.checked_at)}</div>
                     <pre>${escapeHtml(JSON.stringify({ issues: health.issues, checks: health.checks }, null, 2))}</pre>`
                  : ""
              }
              <pre>${escapeHtml(JSON.stringify(provider.config, null, 2))}</pre>
            </div>
          `;
          },
        )
        .join("")
    : `<div class="muted">No stored providers</div>`;
  providerRoot.querySelectorAll("[data-provider-status]").forEach((button) => {
    button.addEventListener("click", () =>
      updateProviderStatus(button.dataset.providerStatus, button.dataset.status),
    );
  });
  providerRoot.querySelectorAll("[data-provider-health]").forEach((button) => {
    button.addEventListener("click", () => checkProviderHealth(button.dataset.providerHealth));
  });
}

async function updateProviderStatus(providerId, status) {
  await api(`/api/providers/${providerId}/status`, {
    method: "PATCH",
    body: JSON.stringify({ status }),
  });
  await refreshOps();
}

async function checkProviderHealth(providerId) {
  const health = await api(`/api/providers/${providerId}/health`);
  state.providerHealth[providerId] = health;
  renderProviders();
}

async function checkVaultHealth() {
  state.vaultHealth = await api("/api/vault/health");
  renderVaultHealth();
}

function renderVaultHealth() {
  vaultHealthRoot.innerHTML = state.vaultHealth
    ? `<div class="item">
        <strong>${escapeHtml(state.vaultHealth.status)}</strong>
        <div class="muted">${escapeHtml(state.vaultHealth.provider_kind)} · ${escapeHtml(state.vaultHealth.healthy ? "healthy" : "unhealthy")}</div>
        <pre>${escapeHtml(JSON.stringify({ issues: state.vaultHealth.issues, checks: state.vaultHealth.checks }, null, 2))}</pre>
      </div>`
    : `<div class="muted">No Vault health check run yet.</div>`;
}

function renderSecretRecords() {
  secretRecordRoot.innerHTML = state.secretRecords.length
    ? state.secretRecords
        .map(
          (secret) => `
            <div class="item">
              <strong>${escapeHtml(secret.name)}</strong>
              <div class="muted">${escapeHtml(secret.scope_type)} · ${escapeHtml(secret.scope_id || "tenant")} · v${escapeHtml(secret.version)}</div>
              <div class="muted">vault:${escapeHtml(secret.path)}#${escapeHtml(secret.key)}</div>
              <button class="secondary" data-secret-rotate="${secret.id}" data-secret-path="${escapeHtml(secret.path)}" data-secret-key="${escapeHtml(secret.key)}">Rotate Ref</button>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No registered secret refs</div>`;
  secretRecordRoot.querySelectorAll("[data-secret-rotate]").forEach((button) => {
    button.addEventListener("click", () =>
      rotateSecretRecord(
        button.dataset.secretRotate,
        button.dataset.secretPath,
        button.dataset.secretKey,
      ),
    );
  });
}

function renderApprovalGovernance() {
  approvalGovernanceRoot.innerHTML = `
    <h4>Approval Groups</h4>
    ${
      state.approvalGroups.length
        ? state.approvalGroups
            .map(
              (group) => `
                <div class="item">
                  <strong>${escapeHtml(group.name)}</strong>
                  <div class="muted">${escapeHtml(group.status)} · ${escapeHtml(group.id)}</div>
                  <div class="muted">${escapeHtml(group.subjects.join(", "))}</div>
                </div>
              `,
            )
            .join("")
        : `<div class="muted">No approval groups</div>`
    }
    <h4>Escalation Rules</h4>
    ${
      state.approvalEscalationRules.length
        ? state.approvalEscalationRules
            .map(
              (rule) => `
                <div class="item">
                  <strong>${escapeHtml(rule.name)}</strong>
                  <div class="muted">${escapeHtml(rule.risk_level)} · group ${escapeHtml(rule.group_id)}</div>
                </div>
              `,
            )
            .join("")
        : `<div class="muted">No escalation rules</div>`
    }
  `;
}

function renderEvalRuns() {
  evalRunRoot.innerHTML = state.evalRuns.length
    ? state.evalRuns
        .map(
          (run) => {
            const gate = state.evalGates[run.id];
            const drift = state.evalDrifts[run.id];
            return `
            <div class="item">
              <strong>${escapeHtml(run.status)} · score ${escapeHtml(run.score ?? "n/a")}</strong>
              <div class="muted">${escapeHtml(run.created_at)}</div>
              <div class="muted">Agent: ${escapeHtml(run.agent_id)} · Version: ${escapeHtml(run.agent_version_id)}</div>
              <button class="secondary" data-eval-gate="${run.id}">Gate 100%</button>
              <button class="secondary" data-eval-drift="${run.id}">Check Drift</button>
              <button class="secondary" data-eval-promote-staging="${run.id}">Promote Staging</button>
              <button class="secondary" data-eval-promote-prod="${run.id}">Promote Prod</button>
              ${
                gate
                  ? `<div class="muted">Gate: ${escapeHtml(gate.status)} · min ${escapeHtml(gate.min_score)}</div>
                     <pre>${escapeHtml(JSON.stringify(gate.failure_reasons, null, 2))}</pre>`
                  : ""
              }
              ${
                drift
                  ? `<div class="muted">Drift: ${escapeHtml(drift.status)} · baseline ${escapeHtml(drift.baseline_run_id || "none")}</div>
                     <pre>${escapeHtml(JSON.stringify(drift.messages, null, 2))}</pre>`
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
  evalRunRoot.querySelectorAll("[data-eval-drift]").forEach((button) => {
    button.addEventListener("click", () => driftEvalRun(button.dataset.evalDrift));
  });
  evalRunRoot.querySelectorAll("[data-eval-promote-staging]").forEach((button) => {
    button.addEventListener("click", () =>
      promoteEvalRun(button.dataset.evalPromoteStaging, "staging"),
    );
  });
  evalRunRoot.querySelectorAll("[data-eval-promote-prod]").forEach((button) => {
    button.addEventListener("click", () =>
      promoteEvalRun(button.dataset.evalPromoteProd, "prod"),
    );
  });
}

function renderAgentReleases() {
  const releaseGroups = state.agents
    .map((agent) => ({
      agent,
      releases: state.agentReleases[agent.id] || [],
    }))
    .filter((group) => group.releases.length);
  agentReleaseRoot.innerHTML = releaseGroups.length
    ? releaseGroups
        .map(
          ({ agent, releases }) => `
            <div class="item">
              <strong>${escapeHtml(agent.name)}</strong>
              <div class="muted">${escapeHtml(agent.id)}</div>
              ${releases
                .map(
                  (release) => `
                    <div class="nested-item">
                      <strong>${escapeHtml(release.environment)} · ${escapeHtml(release.status)}</strong>
                      <div class="muted">Score: ${escapeHtml(release.eval_score ?? "n/a")} · Min: ${escapeHtml(release.min_score)}</div>
                      <div class="muted">Version: ${escapeHtml(release.agent_version_id)}</div>
                      <div class="muted">Eval run: ${escapeHtml(release.eval_run_id || "none")}</div>
                      ${
                        release.status === "promoted"
                          ? `<button class="secondary" data-release-rollback="${release.id}" data-release-agent="${agent.id}">Rollback</button>`
                          : ""
                      }
                    </div>
                  `,
                )
                .join("")}
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No promoted or rolled back releases</div>`;
  agentReleaseRoot.querySelectorAll("[data-release-rollback]").forEach((button) => {
    button.addEventListener("click", () =>
      rollbackAgentRelease(button.dataset.releaseAgent, button.dataset.releaseRollback),
    );
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

function setCodexThreadId(threadId) {
  codexTurnForm.elements.thread_id.value = threadId;
}

function setCodexTurnId(turnId) {
  codexCommandForm.elements.turn_id.value = turnId;
}

function setCodexSyncSessionId(sessionId) {
  codexArtifactSyncForm.elements.session_id.value = sessionId;
}

function populateCodexArtifactSyncFromResponse(response) {
  const artifacts = response?.result?.artifacts || response?.artifacts;
  if (!Array.isArray(artifacts) || !artifacts.length) {
    return;
  }
  codexArtifactSyncForm.elements.artifacts.value = JSON.stringify(artifacts, null, 2);
}

function parseJsonField(value, label) {
  try {
    return JSON.parse(String(value || "{}"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function renderMcpServers() {
  const run = state.mcpHealthRun;
  const runSummary = run
    ? `<div class="item">
        <strong>Team health run</strong>
        <div class="muted">${formatInteger(run.healthy_count)} healthy · ${formatInteger(run.unhealthy_count)} unhealthy · ${formatInteger(run.server_count)} servers · ${escapeHtml(run.checked_at)}</div>
      </div>`
    : "";
  const scheduledRun = state.mcpScheduledHealthRun;
  const scheduledRunSummary = scheduledRun
    ? `<div class="item">
        <strong>Due health run</strong>
        <div class="muted">${formatInteger(scheduledRun.due_count)} due · ${formatInteger(scheduledRun.skipped_count)} skipped · ${formatInteger(scheduledRun.healthy_count)} healthy · ${formatInteger(scheduledRun.unhealthy_count)} unhealthy · ${escapeHtml(scheduledRun.checked_at)}</div>
      </div>`
    : "";
  mcpServerRoot.innerHTML = state.mcpServers.length
    ? `${runSummary}${scheduledRunSummary}${state.mcpServers
        .map((server) => {
          const health = state.mcpHealth[server.id];
          return `
            <div class="item">
              <strong>${escapeHtml(server.name)}</strong>
              <div class="muted">${escapeHtml(server.transport)} · ${escapeHtml(server.status)}</div>
              <div class="muted">Tools: ${escapeHtml(server.tool_allowlist.join(", ") || "none")}</div>
              <button class="secondary" data-edit-mcp="${server.id}">Edit Config</button>
              <button class="secondary" data-health-mcp="${server.id}">Check Health</button>
              <button class="secondary" data-discover-mcp="${server.id}">Discover Tools</button>
              <button class="secondary" data-mcp-status="${server.id}" data-status="active">Activate</button>
              <button class="secondary" data-mcp-status="${server.id}" data-status="disabled">Disable</button>
              <button class="secondary" data-mcp-status="${server.id}" data-status="archived">Archive</button>
              ${
                health
                  ? `<div class="muted">Health: ${health.healthy ? "healthy" : "unhealthy"} · ${escapeHtml(health.issues.join("; ") || "no issues")}</div>`
                  : ""
              }
              <pre>${escapeHtml(JSON.stringify(server.config, null, 2))}</pre>
            </div>
          `;
        })
        .join("")}`
    : state.mcpTeamId
      ? `${runSummary}${scheduledRunSummary}<div class="muted">No MCP servers for this team</div>`
      : `<div class="muted">Enter a team ID to manage MCP servers</div>`;
  mcpServerRoot.querySelectorAll("[data-discover-mcp]").forEach((button) => {
    button.addEventListener("click", () => discoverMcpTools(button.dataset.discoverMcp));
  });
  mcpServerRoot.querySelectorAll("[data-edit-mcp]").forEach((button) => {
    button.addEventListener("click", () => editMcpServer(button.dataset.editMcp));
  });
  mcpServerRoot.querySelectorAll("[data-health-mcp]").forEach((button) => {
    button.addEventListener("click", () => checkMcpHealth(button.dataset.healthMcp));
  });
  mcpServerRoot.querySelectorAll("[data-mcp-status]").forEach((button) => {
    button.addEventListener("click", () =>
      updateMcpStatus(button.dataset.mcpStatus, button.dataset.status),
    );
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
  const delegatedApprover =
    approval.evidence?.approver_subject ??
    approval.evidence?.delegated_approver ??
    approval.evidence?.args?.approver_subject ??
    approval.evidence?.args?.delegated_approver ??
    null;
  const delegatedGroup =
    approval.evidence?.approver_group_name ??
    approval.evidence?.approver_group_id ??
    approval.evidence?.args?.approver_group_id ??
    null;
  const delivery = state.approvalDeliveries[approval.id];
  return `
    <div class="item">
      <strong>${escapeHtml(approval.action)}</strong>
      <div class="muted">${escapeHtml(approval.risk_level)} · ${escapeHtml(approval.status)}</div>
      <div class="muted">Expires: ${escapeHtml(approval.expires_at || "not set")}</div>
      ${delegatedApprover ? `<div class="muted">Delegated approver: ${escapeHtml(delegatedApprover)}</div>` : ""}
      ${delegatedGroup ? `<div class="muted">Delegated group: ${escapeHtml(delegatedGroup)}</div>` : ""}
      <p>${escapeHtml(approval.reason)}</p>
      <dl>
        <dt>Original args</dt>
        <dd><pre>${escapeHtml(JSON.stringify(originalArgs, null, 2))}</pre></dd>
        ${
          modifiedArgs
            ? `<dt>Modified args</dt><dd><pre>${escapeHtml(JSON.stringify(modifiedArgs, null, 2))}</pre></dd>`
            : ""
        }
        ${renderApprovalArgumentDiff(originalArgs, modifiedArgs)}
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
            <button class="secondary" data-deliver-approval="${approval.id}">Deliver</button><button class="secondary" data-escalate-approval="${approval.id}">Escalate</button><button class="secondary" data-approve="${approval.id}">Approve</button><button class="secondary reject" data-reject="${approval.id}">Reject</button><button class="secondary" data-expire="${approval.id}">Expire</button>`
          : ""
      }
      ${
        delivery
          ? `<div class="muted">Delivery: ${escapeHtml(delivery.status)} · ${escapeHtml(delivery.channel)} · ${escapeHtml(delivery.webhook_configured ? "webhook configured" : "webhook not configured")}</div>`
          : ""
      }
    </div>
  `;
}

function renderApprovalArgumentDiff(originalArgs, modifiedArgs) {
  if (!modifiedArgs) return "";
  const rows = jsonDiffRows(originalArgs, modifiedArgs);
  const body = rows.length
    ? `<table class="diff-table">
        <thead>
          <tr>
            <th>Change</th>
            <th>Path</th>
            <th>Original</th>
            <th>Modified</th>
          </tr>
        </thead>
        <tbody>
          ${rows
            .map(
              (row) => `
                <tr>
                  <td><span class="diff-kind ${escapeHtml(row.kind)}">${escapeHtml(row.kind)}</span></td>
                  <td><code>${escapeHtml(row.path)}</code></td>
                  <td><pre>${escapeHtml(formatDiffValue(row.before, row.beforeMissing))}</pre></td>
                  <td><pre>${escapeHtml(formatDiffValue(row.after, row.afterMissing))}</pre></td>
                </tr>
              `,
            )
            .join("")}
        </tbody>
      </table>`
    : `<div class="muted">No argument changes detected.</div>`;
  return `<dt>Argument diff</dt><dd>${body}</dd>`;
}

function jsonDiffRows(before, after, path = "$", beforeMissing = false, afterMissing = false) {
  if (beforeMissing) {
    return [{ kind: "added", path, before, after, beforeMissing, afterMissing }];
  }
  if (afterMissing) {
    return [{ kind: "removed", path, before, after, beforeMissing, afterMissing }];
  }
  if (jsonEqual(before, after)) {
    return [];
  }
  if (Array.isArray(before) && Array.isArray(after)) {
    const maxLength = Math.max(before.length, after.length);
    return Array.from({ length: maxLength }, (_, index) =>
      jsonDiffRows(before[index], after[index], `${path}[${index}]`, index >= before.length, index >= after.length),
    ).flat();
  }
  if (isPlainObject(before) && isPlainObject(after)) {
    const keys = Array.from(new Set([...Object.keys(before), ...Object.keys(after)])).sort();
    return keys
      .map((key) =>
        jsonDiffRows(
          before[key],
          after[key],
          appendJsonPath(path, key),
          !Object.prototype.hasOwnProperty.call(before, key),
          !Object.prototype.hasOwnProperty.call(after, key),
        ),
      )
      .flat();
  }
  return [{ kind: "changed", path, before, after, beforeMissing, afterMissing }];
}

function appendJsonPath(path, key) {
  return /^[A-Za-z_$][\w$]*$/.test(key) ? `${path}.${key}` : `${path}[${JSON.stringify(key)}]`;
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function jsonEqual(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

function formatDiffValue(value, missing) {
  if (missing) return "(missing)";
  return JSON.stringify(value, null, 2);
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
