const state = {
  agents: [],
  session: null,
  events: [],
  artifacts: [],
  toolCalls: [],
  auditLogs: [],
  providers: [],
  providerSummary: null,
  providerPolicyGate: null,
  providerPolicyGateRuns: null,
  providerDeploymentValidation: null,
  providerProductionRolloutRun: null,
  providerHealth: {},
  vaultHealth: null,
  vaultReadiness: null,
  vaultKmsRotationRun: null,
  secretRecords: [],
  policy: null,
  policyDecision: null,
  policyRuntime: null,
  policyTest: null,
  policyRevisions: [],
  policyRevisionDiffs: {},
  policyRevisionGates: {},
  policyScheduledRolloutRun: null,
  policyRollback: null,
  evalDatasets: [],
  evalCases: [],
  evalRuns: [],
  evalJudgeProfiles: [],
  evalSuiteBootstrap: null,
  evalGates: {},
  evalDrifts: {},
  agentReleases: {},
  agentReleaseSummary: null,
  agentReleaseAutomationRuns: null,
  agentReleaseAutomationRun: null,
  mcpServers: [],
  mcpTeamId: "",
  mcpHealth: {},
  mcpHealthRun: null,
  mcpScheduledHealthRun: null,
  mcpDeploymentValidation: null,
  mcpRolloutRun: null,
  mcpRolloutSummary: null,
  mcpRolloutRuns: null,
  executionJobs: [],
  workerReadiness: null,
  workerLoadValidationRun: null,
  remoteComputerReadiness: null,
  remoteComputerRunnerReadiness: null,
  remoteComputers: [],
  remoteComputerLeases: [],
  remoteComputerAttachments: [],
  remoteComputerStateLocks: [],
  remoteComputerSidecarHeartbeats: [],
  remoteComputerArtifactDiscovery: null,
  remoteComputerSidecarRecoveryRun: null,
  usageRollups: [],
  usageTrend: null,
  usageFinanceSummary: null,
  usageFinanceOperations: null,
  costAlertRoutes: [],
  usage: null,
  observability: null,
  observabilityCollectorReadiness: null,
  observabilityRemediationPlan: null,
  observabilityRemediation: null,
  usageFinanceOperationsRun: null,
  schedulerSummary: null,
  schedulerDuePlan: null,
  schedulerDueRun: null,
  costAlertDelivery: null,
  costAlertAcknowledgement: null,
  usageExportStatus: null,
  usageExportDelivery: null,
  organizations: [],
  teams: [],
  projects: [],
  memberships: [],
  tenantInvitations: [],
  tenantIsolationReadiness: null,
  tenantProvisioning: null,
  selectedOrganizationId: "",
  selectedTeamId: "",
  approvalDeliveries: {},
  approvalNotificationRouting: null,
  approvalNotificationRuns: null,
  approvalNotificationRun: null,
  approvalNotificationDeploymentValidation: null,
  approvalNotificationChannelPolicies: [],
  approvalEscalationDueRun: null,
  approvalGroups: [],
  approvalEscalationRules: [],
  codexAppServer: {
    health: null,
    thread: null,
    turn: null,
    command: null,
    interrupt: null,
    stalePoll: null,
    sync: null,
    runs: [],
    traces: null,
    controlSummary: null,
    traceDetail: null,
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
const vaultReadinessRoot = document.querySelector("#vault-readiness");
const secretRecordRoot = document.querySelector("#secret-records");
const checkVaultHealthButton = document.querySelector("#check-vault-health");
const runVaultKmsRotationButton = document.querySelector("#run-vault-kms-rotation");
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
const runDuePolicyRolloutsButton = document.querySelector("#run-due-policy-rollouts");
const rollbackPolicyRolloutButton = document.querySelector("#rollback-policy-rollout");
const evalDatasetRoot = document.querySelector("#eval-datasets");
const evalCaseRoot = document.querySelector("#eval-cases");
const evalRunRoot = document.querySelector("#eval-runs");
const evalJudgeProfileRoot = document.querySelector("#eval-judge-profiles");
const evalSuiteBootstrapRoot = document.querySelector("#eval-suite-bootstrap");
const agentReleaseRoot = document.querySelector("#agent-releases");
const runDueAgentReleasesButton = document.querySelector("#run-due-agent-releases");
const mcpServerRoot = document.querySelector("#mcp-servers");
const executionJobRoot = document.querySelector("#execution-jobs");
const workerReadinessRoot = document.querySelector("#worker-readiness");
const runWorkerLoadValidationButton = document.querySelector("#run-worker-load-validation");
const remoteComputerReadinessRoot = document.querySelector("#remote-computer-readiness");
const remoteArtifactDiscoveryForm = document.querySelector("#remote-artifact-discovery-form");
const remoteStateLockForm = document.querySelector("#remote-state-lock-form");
const usageRoot = document.querySelector("#usage-summary");
const observabilityRoot = document.querySelector("#observability-summary");
const validateObservabilityCollectorButton = document.querySelector("#validate-observability-collector");
const runObservabilityRemediationButton = document.querySelector("#run-observability-remediation");
const runSchedulerDueButton = document.querySelector("#run-scheduler-due");
const usageRollupRoot = document.querySelector("#usage-rollups");
const costAlertRouteRoot = document.querySelector("#cost-alert-routes");
const governanceRoot = document.querySelector("#governance-status");
const organizationRoot = document.querySelector("#organizations");
const tenantIsolationReadinessRoot = document.querySelector("#tenant-isolation-readiness");
const teamRoot = document.querySelector("#teams");
const projectRoot = document.querySelector("#projects");
const membershipRoot = document.querySelector("#memberships");
const tenantInvitationRoot = document.querySelector("#tenant-invitations");
const agentForm = document.querySelector("#agent-form");
const organizationForm = document.querySelector("#organization-form");
const tenantProvisioningForm = document.querySelector("#tenant-provisioning-form");
const organizationOwnerForm = document.querySelector("#organization-owner-form");
const teamForm = document.querySelector("#team-form");
const projectForm = document.querySelector("#project-form");
const membershipForm = document.querySelector("#membership-form");
const tenantInvitationForm = document.querySelector("#tenant-invitation-form");
const approvalGroupForm = document.querySelector("#approval-group-form");
const approvalEscalationRuleForm = document.querySelector("#approval-escalation-rule-form");
const approvalNotificationChannelPolicyForm = document.querySelector(
  "#approval-notification-channel-policy-form",
);
const runDueApprovalEscalationsButton = document.querySelector("#run-due-approval-escalations");
const runApprovalNotificationsButton = document.querySelector("#run-approval-notifications");
const validateApprovalNotificationsButton = document.querySelector("#validate-approval-notifications");
const approvalGovernanceRoot = document.querySelector("#approval-governance");
const approvalNotificationRoutingRoot = document.querySelector("#approval-notification-routing");
const approvalNotificationRunsRoot = document.querySelector("#approval-notification-runs");
const providerForm = document.querySelector("#provider-form");
const providerStatusApprovalForm = document.querySelector("#provider-status-approval-form");
const runProviderPolicyGateButton = document.querySelector("#run-provider-policy-gate");
const validateProviderDeploymentButton = document.querySelector("#validate-provider-deployment");
const runProviderProductionRolloutButton = document.querySelector("#run-provider-production-rollout");
const secretForm = document.querySelector("#secret-form");
const evalJudgeProfileForm = document.querySelector("#eval-judge-profile-form");
const evalDatasetForm = document.querySelector("#eval-dataset-form");
const evalCaseForm = document.querySelector("#eval-case-form");
const evalRunForm = document.querySelector("#eval-run-form");
const bootstrapEvalSuiteButton = document.querySelector("#bootstrap-eval-suite");
const mcpForm = document.querySelector("#mcp-form");
const loadMcpButton = document.querySelector("#load-mcp");
const runMcpHealthButton = document.querySelector("#run-mcp-health");
const runDueMcpHealthButton = document.querySelector("#run-due-mcp-health");
const validateMcpDeploymentButton = document.querySelector("#validate-mcp-deployment");
const runDueMcpRolloutsButton = document.querySelector("#run-due-mcp-rollouts");
const loadEvalCasesButton = document.querySelector("#load-eval-cases");
const refreshExecutionJobsButton = document.querySelector("#refresh-execution-jobs");
const createUsageRollupButton = document.querySelector("#create-usage-rollup");
const deliverCostAlertsButton = document.querySelector("#deliver-cost-alerts");
const runFinanceOperationsButton = document.querySelector("#run-finance-operations");
const exportUsageCsvButton = document.querySelector("#export-usage-csv");
const deliverUsageExportButton = document.querySelector("#deliver-usage-export");
const costAlertRouteForm = document.querySelector("#cost-alert-route-form");
const checkCodexHealthButton = document.querySelector("#check-codex-health");
const validateCodexDeploymentButton = document.querySelector("#validate-codex-deployment");
const loadCodexRunsButton = document.querySelector("#load-codex-runs");
const pollStaleCodexRunsButton = document.querySelector("#poll-stale-codex-runs");
const codexThreadForm = document.querySelector("#codex-thread-form");
const codexTurnForm = document.querySelector("#codex-turn-form");
const codexCommandForm = document.querySelector("#codex-command-form");
const codexArtifactSyncForm = document.querySelector("#codex-artifact-sync-form");
const interruptCodexTurnButton = document.querySelector("#interrupt-codex-turn");
const codexAppServerRoot = document.querySelector("#codex-app-server");

document.querySelector("#new-session").addEventListener("click", runDemo);
agentForm.addEventListener("submit", createAgent);
organizationForm.addEventListener("submit", createOrganization);
tenantProvisioningForm.addEventListener("submit", bootstrapTenantProvisioning);
organizationOwnerForm.addEventListener("submit", transferOrganizationOwnership);
teamForm.addEventListener("submit", createTeam);
projectForm.addEventListener("submit", createProject);
membershipForm.addEventListener("submit", createMembership);
tenantInvitationForm.addEventListener("submit", createTenantInvitation);
approvalGroupForm.addEventListener("submit", createApprovalGroup);
approvalEscalationRuleForm.addEventListener("submit", createApprovalEscalationRule);
approvalNotificationChannelPolicyForm.addEventListener(
  "submit",
  createApprovalNotificationChannelPolicy,
);
runDueApprovalEscalationsButton.addEventListener("click", runDueApprovalEscalations);
runApprovalNotificationsButton.addEventListener("click", runApprovalNotifications);
validateApprovalNotificationsButton.addEventListener("click", validateApprovalNotifications);
providerForm.addEventListener("submit", createProvider);
providerStatusApprovalForm.addEventListener("submit", requestProviderStatusApproval);
runProviderPolicyGateButton.addEventListener("click", runProviderPolicyGate);
validateProviderDeploymentButton.addEventListener("click", validateProviderDeployment);
runProviderProductionRolloutButton.addEventListener("click", runProviderProductionRollout);
secretForm.addEventListener("submit", createSecretRecord);
evalJudgeProfileForm.addEventListener("submit", createEvalJudgeProfile);
checkVaultHealthButton.addEventListener("click", checkVaultHealth);
runVaultKmsRotationButton.addEventListener("click", runVaultKmsRotation);
policyForm.addEventListener("submit", simulatePolicy);
policyTestForm.addEventListener("submit", testPolicy);
policyRevisionForm.addEventListener("submit", createPolicyRevision);
cancelPolicyRolloutButton.addEventListener("click", cancelPolicyRollout);
runDuePolicyRolloutsButton.addEventListener("click", runDuePolicyRollouts);
rollbackPolicyRolloutButton.addEventListener("click", rollbackPolicyRollout);
evalDatasetForm.addEventListener("submit", createEvalDataset);
evalCaseForm.addEventListener("submit", createEvalCase);
evalRunForm.addEventListener("submit", createEvalRun);
bootstrapEvalSuiteButton.addEventListener("click", bootstrapEvalSuite);
runDueAgentReleasesButton.addEventListener("click", runDueAgentReleaseAutomation);
mcpForm.addEventListener("submit", createMcpServer);
loadMcpButton.addEventListener("click", loadMcpServers);
runMcpHealthButton.addEventListener("click", runMcpHealth);
runDueMcpHealthButton.addEventListener("click", runDueMcpHealth);
validateMcpDeploymentButton.addEventListener("click", validateMcpDeployment);
runDueMcpRolloutsButton.addEventListener("click", runDueMcpRollouts);
loadEvalCasesButton.addEventListener("click", loadEvalCases);
refreshExecutionJobsButton.addEventListener("click", refreshExecutionJobs);
runWorkerLoadValidationButton.addEventListener("click", runWorkerLoadValidation);
remoteArtifactDiscoveryForm.addEventListener("submit", discoverRemoteArtifacts);
remoteStateLockForm.addEventListener("submit", acquireRemoteStateLock);
createUsageRollupButton.addEventListener("click", createUsageRollup);
deliverCostAlertsButton.addEventListener("click", deliverCostAlerts);
runFinanceOperationsButton.addEventListener("click", runFinanceOperations);
exportUsageCsvButton.addEventListener("click", exportUsageCsv);
deliverUsageExportButton.addEventListener("click", deliverUsageExport);
validateObservabilityCollectorButton.addEventListener("click", validateObservabilityCollector);
runObservabilityRemediationButton.addEventListener("click", runObservabilityRemediation);
runSchedulerDueButton.addEventListener("click", runSchedulerDueTasks);
costAlertRouteForm.addEventListener("submit", createCostAlertRoute);
checkCodexHealthButton.addEventListener("click", checkCodexAppServerHealth);
validateCodexDeploymentButton.addEventListener("click", validateCodexDeployment);
loadCodexRunsButton.addEventListener("click", loadCodexAppServerRuns);
pollStaleCodexRunsButton.addEventListener("click", pollStaleCodexRuns);
codexThreadForm.addEventListener("submit", createCodexThread);
codexTurnForm.addEventListener("submit", createCodexTurn);
codexCommandForm.addEventListener("submit", executeCodexCommand);
codexArtifactSyncForm.addEventListener("submit", syncCodexArtifacts);
interruptCodexTurnButton.addEventListener("click", interruptCodexTurn);

async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: adminHeaders(),
    ...options,
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return response.json();
}

function adminHeaders(extra = {}) {
  return {
    "content-type": "application/json",
    "x-mandoforge-subject": "web-admin",
    "x-mandoforge-roles": "admin",
    ...extra,
  };
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

async function bootstrapTenantProvisioning(event) {
  event.preventDefault();
  const form = new FormData(tenantProvisioningForm);
  state.tenantProvisioning = await api("/api/tenant-provisioning/bootstrap", {
    method: "POST",
    body: JSON.stringify({
      owner_subject: String(form.get("owner_subject") || "").trim(),
      organization_name: String(form.get("organization_name") || "").trim(),
      organization_slug: String(form.get("organization_slug") || "").trim(),
      team_name: String(form.get("team_name") || "").trim(),
      team_slug: String(form.get("team_slug") || "").trim(),
      owner_role: "admin",
    }),
  });
  setOrganizationId(state.tenantProvisioning.organization.id);
  if (state.tenantProvisioning.team) {
    setTeamId(state.tenantProvisioning.team.id);
  }
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

async function createTenantInvitation(event) {
  event.preventDefault();
  const form = new FormData(tenantInvitationForm);
  const organizationId = String(
    form.get("organization_id") || state.selectedOrganizationId,
  ).trim();
  const payload = {
    email: String(form.get("email") || "").trim(),
    role: String(form.get("role") || "").trim(),
    expires_in_hours: Number(form.get("expires_in_hours") || 168),
  };
  const teamId = String(form.get("team_id") || "").trim();
  const projectId = String(form.get("project_id") || "").trim();
  if (teamId) payload.team_id = teamId;
  if (projectId) payload.project_id = projectId;
  await api(`/api/organizations/${organizationId}/invitations`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
  setOrganizationId(organizationId);
  await refreshOps();
}

async function revokeTenantInvitation(invitationId) {
  await api(`/api/invitations/${invitationId}/revoke`, { method: "POST" });
  await refreshOps();
}

async function archiveOrganization(organizationId) {
  await api(`/api/organizations/${organizationId}/archive`, { method: "POST" });
  if (state.selectedOrganizationId === organizationId) {
    state.selectedOrganizationId = "";
    state.selectedTeamId = "";
  }
  await refreshOps();
}

async function transferOrganizationOwnership(event) {
  event.preventDefault();
  const form = new FormData(organizationOwnerForm);
  const organizationId = String(form.get("organization_id") || "").trim();
  await api(`/api/organizations/${organizationId}/transfer-ownership`, {
    method: "POST",
    body: JSON.stringify({
      owner_subject: String(form.get("owner_subject") || "").trim(),
    }),
  });
  await refreshOps();
}

async function deleteOrganization(organizationId) {
  await api(`/api/organizations/${organizationId}`, { method: "DELETE" });
  if (state.selectedOrganizationId === organizationId) {
    state.selectedOrganizationId = "";
    state.selectedTeamId = "";
  }
  await refreshOps();
}

async function archiveTeam(teamId) {
  await api(`/api/teams/${teamId}/archive`, { method: "POST" });
  if (state.selectedTeamId === teamId) {
    state.selectedTeamId = "";
  }
  await refreshOps();
}

async function archiveProject(projectId) {
  await api(`/api/projects/${projectId}/archive`, { method: "POST" });
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

async function createApprovalNotificationChannelPolicy(event) {
  event.preventDefault();
  const form = new FormData(approvalNotificationChannelPolicyForm);
  await api("/api/approvals/notification-channel-policies", {
    method: "POST",
    body: JSON.stringify({
      name: String(form.get("name") || "").trim(),
      channel: String(form.get("channel") || "").trim(),
      target_env: String(form.get("target_env") || "").trim() || null,
      risk_filter: String(form.get("risk_filter") || "").trim() || "all",
      max_attempts: Number(form.get("max_attempts") || 1),
      backoff_seconds: Number(form.get("backoff_seconds") || 0),
    }),
  });
  await refreshOps();
}

async function archiveApprovalNotificationChannelPolicy(id) {
  await api(`/api/approvals/notification-channel-policies/${id}/archive`, {
    method: "POST",
  });
  await refreshOps();
}

async function runDueApprovalEscalations() {
  state.approvalEscalationDueRun = await api("/api/approvals/escalations/run-due", {
    method: "POST",
  });
  await refreshApprovals();
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

async function requestProviderStatusApproval(event) {
  event.preventDefault();
  const form = new FormData(providerStatusApprovalForm);
  const providerId = String(form.get("provider_id") || "").trim();
  if (!providerId) {
    throw new Error("Provider ID is required");
  }
  await api(`/api/providers/${providerId}/status-approval`, {
    method: "POST",
    body: JSON.stringify({
      status: String(form.get("status") || "").trim(),
      reason: String(form.get("reason") || "").trim(),
      approver_subject: String(form.get("approver_subject") || "").trim(),
    }),
  });
  await refreshOps();
}

async function runProviderPolicyGate() {
  const result = await api("/api/providers/policy-gate/run", {
    method: "POST",
  });
  state.providerPolicyGate = result.report;
  state.providerPolicyGateRuns = await api("/api/providers/policy-gate/runs");
  renderProviders();
}

async function validateProviderDeployment() {
  state.providerDeploymentValidation = await api("/api/providers/deployment/validate", {
    method: "POST",
  });
  state.providerSummary = await api("/api/providers/summary");
  state.providerHealth = Object.fromEntries(
    (state.providerDeploymentValidation.results || []).map((health) => [health.provider_id, health]),
  );
  renderProviders();
}

async function runProviderProductionRollout() {
  state.providerProductionRolloutRun = await api("/api/providers/production-rollout/run", {
    method: "POST",
    body: JSON.stringify({
      environment: "production",
      reason: "Static console production rollout gate check",
    }),
  });
  state.providerPolicyGateRuns = await api("/api/providers/policy-gate/runs");
  renderProviders();
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
  const value = String(form.get("value") || "");
  if (value) payload.value = value;
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

async function runDuePolicyRollouts() {
  state.policyScheduledRolloutRun = await api("/api/policy/rollout/run-due", { method: "POST" });
  state.policyRuntime = await api("/api/policy/runtime");
  state.policyRevisions = await api("/api/policy/revisions");
  renderPolicy();
}

async function rollbackPolicyRollout() {
  state.policyRollback = await api("/api/policy/rollout/rollback", { method: "POST" });
  state.policyRuntime = await api("/api/policy/runtime");
  state.policyRevisions = await api("/api/policy/revisions");
  state.policy = await api("/api/policy");
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

async function createEvalJudgeProfile(event) {
  event.preventDefault();
  const form = new FormData(evalJudgeProfileForm);
  await api("/api/eval/judge-profiles", {
    method: "POST",
    body: JSON.stringify({
      name: String(form.get("name") || "").trim(),
      endpoint: String(form.get("endpoint") || "").trim(),
      model: String(form.get("model") || "").trim(),
      api_key_ref: String(form.get("api_key_ref") || "").trim() || null,
      timeout_seconds: Number(form.get("timeout_seconds") || 30),
    }),
  });
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

async function bootstrapEvalSuite() {
  const judgeProfile = state.evalJudgeProfiles[0]?.name || "";
  state.evalSuiteBootstrap = await api("/api/eval/suites/stage2-regression", {
    method: "POST",
    body: JSON.stringify({
      judge_profile: judgeProfile || null,
    }),
  });
  setEvalDatasetId(state.evalSuiteBootstrap.dataset.id);
  state.evalCases = state.evalSuiteBootstrap.cases;
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

async function requestEvalRunPromotion(runId) {
  const run = state.evalRuns.find((candidate) => candidate.id === runId);
  if (!run) return;
  const activateAfter = new Date(Date.now() - 60_000).toISOString();
  await api(`/api/agents/${run.agent_id}/release-requests`, {
    method: "POST",
    body: JSON.stringify({
      eval_run_id: run.id,
      agent_version_id: run.agent_version_id,
      environment: "prod",
      min_score: 1.0,
      approver_subject: "release-approver-1",
      reason: "Production release requires separation of duties",
      auto_approve: false,
      activate_after: activateAfter,
    }),
  });
  await refreshAgentReleases();
}

async function requestEvalRunAutoPromotion(runId) {
  const run = state.evalRuns.find((candidate) => candidate.id === runId);
  if (!run) return;
  const activateAfter = new Date(Date.now() - 60_000).toISOString();
  await api(`/api/agents/${run.agent_id}/release-requests`, {
    method: "POST",
    body: JSON.stringify({
      eval_run_id: run.id,
      agent_version_id: run.agent_version_id,
      environment: "prod",
      min_score: 1.0,
      approver_subject: "system",
      reason: "Production release is eligible for system due-run automation",
      auto_approve: true,
      activate_after: activateAfter,
    }),
  });
  await refreshAgentReleases();
}

async function runDueAgentReleaseAutomation() {
  state.agentReleaseAutomationRun = await api("/api/agents/releases/run-due", {
    method: "POST",
  });
  await refreshAgentReleases();
}

async function approveAgentRelease(agentId, releaseId) {
  await api(`/api/agents/${agentId}/releases/${releaseId}/approve`, {
    method: "POST",
  });
  await refreshAgentReleases();
}

async function rejectAgentRelease(agentId, releaseId) {
  await api(`/api/agents/${agentId}/releases/${releaseId}/reject`, {
    method: "POST",
    body: JSON.stringify({ reason: "Rejected from static console" }),
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

async function runFinanceOperations() {
  state.usageFinanceOperationsRun = await api("/api/usage/finance-operations/run", {
    method: "POST",
  });
  await refreshOps();
}

async function exportUsageCsv() {
  const response = await fetch("/api/usage/export.csv", {
    headers: adminHeaders({ accept: "text/csv" }),
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  const csv = await response.text();
  state.usageExportStatus = {
    status: "ready",
    bytes: csv.length,
    preview: csv.split("\n").slice(0, 6).join("\n"),
  };
  renderUsage();
}

async function deliverUsageExport() {
  state.usageExportDelivery = await api("/api/usage/export/deliver", {
    method: "POST",
  });
  await refreshOps();
}

async function validateObservabilityCollector() {
  state.observabilityCollectorValidation = await api("/api/observability/collector/deployment/validate", {
    method: "POST",
  });
  await refreshOps();
}

async function runObservabilityRemediation() {
  state.observabilityRemediation = await api("/api/observability/remediation/run", {
    method: "POST",
  });
  await refreshApprovals();
  await refreshOps();
}

async function runSchedulerDueTasks() {
  state.schedulerDueRun = await api("/api/scheduler/run-due", {
    method: "POST",
  });
  await refreshApprovals();
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
    const [health, controlSummary] = await Promise.all([
      api("/api/codex-app-server/health"),
      api("/api/codex-app-server/control-plane/summary"),
    ]);
    state.codexAppServer.health = health;
    state.codexAppServer.controlSummary = controlSummary;
  });
}

async function validateCodexDeployment() {
  await captureCodexAppServer("deploymentValidation", async () => {
    state.codexAppServer.deploymentValidation = await api("/api/codex-app-server/deployment/validate", {
      method: "POST",
    });
    state.codexAppServer.controlSummary = await api("/api/codex-app-server/control-plane/summary");
  });
}

async function loadCodexAppServerRuns() {
  await captureCodexAppServer("runs", async () => {
    const [runs, traces, controlSummary] = await Promise.all([
      api("/api/codex-app-server/runs"),
      api("/api/codex-app-server/traces"),
      api("/api/codex-app-server/control-plane/summary"),
    ]);
    state.codexAppServer.runs = runs;
    state.codexAppServer.traces = traces;
    state.codexAppServer.controlSummary = controlSummary;
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
    const [runs, traces, controlSummary] = await Promise.all([
      api("/api/codex-app-server/runs"),
      api("/api/codex-app-server/traces"),
      api("/api/codex-app-server/control-plane/summary"),
    ]);
    state.codexAppServer.runs = runs;
    state.codexAppServer.traces = traces;
    state.codexAppServer.controlSummary = controlSummary;
  });
}

async function pollStaleCodexRuns() {
  await captureCodexAppServer("stalePoll", async () => {
    state.codexAppServer.stalePoll = await api("/api/codex-app-server/runs/poll-stale", {
      method: "POST",
      body: JSON.stringify({
        stale_after_seconds: 300,
        max_attempts: 1,
        retry_interval_ms: 0,
        max_runs: 20,
      }),
    });
    const [runs, traces, controlSummary] = await Promise.all([
      api("/api/codex-app-server/runs"),
      api("/api/codex-app-server/traces"),
      api("/api/codex-app-server/control-plane/summary"),
    ]);
    state.codexAppServer.runs = runs;
    state.codexAppServer.traces = traces;
    state.codexAppServer.controlSummary = controlSummary;
  });
}

async function loadCodexTraceDetail(traceKey) {
  await captureCodexAppServer("traceDetail", async () => {
    state.codexAppServer.traceDetail = await api(
      `/api/codex-app-server/traces/${encodeURIComponent(traceKey)}`,
    );
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
  const [servers, rolloutSummary, rolloutRuns] = await Promise.all([
    api(`/api/teams/${teamId}/mcp-servers`),
    api(`/api/teams/${teamId}/mcp-servers/rollouts/summary`),
    api(`/api/teams/${teamId}/mcp-servers/rollouts/runs`),
  ]);
  state.mcpServers = servers;
  state.mcpRolloutSummary = rolloutSummary;
  state.mcpRolloutRuns = rolloutRuns;
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

async function validateMcpDeployment() {
  if (!state.mcpTeamId) {
    const form = new FormData(mcpForm);
    state.mcpTeamId = String(form.get("team_id") || "").trim();
  }
  if (!state.mcpTeamId) return;
  state.mcpDeploymentValidation = await api(`/api/teams/${state.mcpTeamId}/mcp-servers/deployment/validate`, {
    method: "POST",
  });
  state.mcpHealth = Object.fromEntries((state.mcpDeploymentValidation.results || []).map((health) => [health.server_id, health]));
  await loadMcpServers();
}

async function runDueMcpRollouts() {
  if (!state.mcpTeamId) {
    const form = new FormData(mcpForm);
    state.mcpTeamId = String(form.get("team_id") || "").trim();
  }
  if (!state.mcpTeamId) return;
  state.mcpRolloutRun = await api(`/api/teams/${state.mcpTeamId}/mcp-servers/rollouts/run-due`, {
    method: "POST",
  });
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

async function requestMcpRollout(serverId) {
  if (!state.mcpTeamId) return;
  const server = state.mcpServers.find((item) => item.id === serverId);
  if (!server) return;
  const transport = window.prompt("Target transport", server.transport);
  if (transport === null) return;
  const tools = window.prompt("Target tool allowlist", server.tool_allowlist.join(","));
  if (tools === null) return;
  const config = window.prompt("Target config JSON", JSON.stringify(server.config, null, 2));
  if (config === null) return;
  const status = window.prompt("Target status", server.status);
  if (status === null) return;
  const activateAfter = window.prompt("Activate after RFC3339 (blank for manual apply)", "");
  if (activateAfter === null) return;
  await api(`/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}/rollouts`, {
    method: "POST",
    body: JSON.stringify({
      transport,
      config: parseJsonField(config, "MCP rollout config"),
      tool_allowlist: tools
        .split(",")
        .map((tool) => tool.trim())
        .filter(Boolean),
      status,
      activate_after: activateAfter.trim() || null,
      reason: "Requested from static console",
    }),
  });
  await loadMcpServers();
}

async function applyMcpRollout(serverId, rolloutId) {
  if (!state.mcpTeamId) return;
  await api(`/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}/rollouts/${rolloutId}/apply`, {
    method: "POST",
  });
  await loadMcpServers();
}

async function rollbackMcpRollout(serverId, rolloutId) {
  if (!state.mcpTeamId) return;
  await api(`/api/teams/${state.mcpTeamId}/mcp-servers/${serverId}/rollouts/${rolloutId}/rollback`, {
    method: "POST",
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
  const [executionJobs, workerReadiness] = await Promise.all([
    api("/api/execution-jobs"),
    api("/api/execution-jobs/worker-readiness"),
  ]);
  state.executionJobs = executionJobs;
  state.workerReadiness = workerReadiness;
  renderWorkerReadiness();
  renderExecutionJobs();
}

async function runWorkerLoadValidation() {
  state.workerLoadValidationRun = await api("/api/execution-jobs/worker-load-validation/run", {
    method: "POST",
  });
  await refreshExecutionJobs();
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
    providerSummary,
    providerPolicyGate,
    providerPolicyGateRuns,
    vaultReadiness,
    secretRecords,
    policy,
    policyRuntime,
    policyRevisions,
    evalJudgeProfiles,
    evalDatasets,
    evalRuns,
    usage,
    usageTrend,
    usageFinanceSummary,
    usageFinanceOperations,
    observability,
    observabilityCollectorReadiness,
    observabilityRemediationPlan,
    schedulerSummary,
    schedulerDuePlan,
    usageRollups,
    costAlertRoutes,
    tenantIsolationReadiness,
    organizations,
    executionJobs,
    workerReadiness,
    remoteComputerReadiness,
    remoteComputerRunnerReadiness,
    remoteComputers,
    remoteComputerLeases,
    remoteComputerAttachments,
    remoteComputerJobAssignments,
    remoteComputerStateLocks,
    remoteComputerSidecarHeartbeats,
    approvalGroups,
    approvalEscalationRules,
    approvalNotificationChannelPolicies,
    approvalNotificationRouting,
    approvalNotificationRuns,
  ] =
    await Promise.all([
      api("/api/providers"),
      api("/api/providers/summary"),
      api("/api/providers/policy-gate"),
      api("/api/providers/policy-gate/runs"),
      api("/api/vault/readiness"),
      api("/api/vault/secrets"),
      api("/api/policy"),
      api("/api/policy/runtime"),
      api("/api/policy/revisions"),
      api("/api/eval/judge-profiles"),
      api("/api/eval/datasets"),
      api("/api/eval/runs"),
      api("/api/usage"),
      api("/api/usage/trends"),
      api("/api/usage/finance-summary"),
      api("/api/usage/finance-operations/summary"),
      api("/api/observability"),
      api("/api/observability/collector-readiness"),
      api("/api/observability/remediation/plan"),
      api("/api/scheduler/summary"),
      api("/api/scheduler/due-plan"),
      api("/api/usage/rollups"),
      api("/api/usage/alert-routes"),
      api("/api/tenant-isolation/readiness"),
      api("/api/organizations"),
      api("/api/execution-jobs"),
      api("/api/execution-jobs/worker-readiness"),
      api("/api/remote-computers/readiness"),
      api("/api/remote-computers/runner/readiness"),
      api("/api/remote-computers"),
      api("/api/remote-computer-leases"),
      api("/api/remote-computer-attachments"),
      api("/api/remote-computer-job-assignments"),
      api("/api/remote-computers/state-locks"),
      api("/api/remote-computers/sidecars/heartbeats"),
      api("/api/approval-groups"),
      api("/api/approval-escalation-rules"),
      api("/api/approvals/notification-channel-policies"),
      api("/api/approvals/notification-routing/summary"),
      api("/api/approvals/notifications/runs"),
    ]);
  state.providers = providers;
  state.providerSummary = providerSummary;
  state.providerPolicyGate = providerPolicyGate;
  state.providerPolicyGateRuns = providerPolicyGateRuns;
  state.vaultReadiness = vaultReadiness;
  state.secretRecords = secretRecords;
  state.policy = policy;
  state.policyRuntime = policyRuntime;
  state.policyRevisions = policyRevisions;
  state.evalJudgeProfiles = evalJudgeProfiles;
  state.evalDatasets = evalDatasets;
  state.evalRuns = evalRuns;
  state.usage = usage;
  state.usageTrend = usageTrend;
  state.usageFinanceSummary = usageFinanceSummary;
  state.usageFinanceOperations = usageFinanceOperations;
  state.observability = observability;
  state.observabilityCollectorReadiness = observabilityCollectorReadiness;
  state.observabilityRemediationPlan = observabilityRemediationPlan;
  state.schedulerSummary = schedulerSummary;
  state.schedulerDuePlan = schedulerDuePlan;
  state.usageRollups = usageRollups;
  state.costAlertRoutes = costAlertRoutes;
  state.tenantIsolationReadiness = tenantIsolationReadiness;
  state.organizations = organizations;
  state.executionJobs = executionJobs;
  state.workerReadiness = workerReadiness;
  state.remoteComputerReadiness = remoteComputerReadiness;
  state.remoteComputerRunnerReadiness = remoteComputerRunnerReadiness;
  state.remoteComputers = remoteComputers;
  state.remoteComputerLeases = remoteComputerLeases;
  state.remoteComputerAttachments = remoteComputerAttachments;
  state.remoteComputerJobAssignments = remoteComputerJobAssignments;
  state.remoteComputerStateLocks = remoteComputerStateLocks;
  state.remoteComputerSidecarHeartbeats = remoteComputerSidecarHeartbeats;
  state.approvalGroups = approvalGroups;
  state.approvalEscalationRules = approvalEscalationRules;
  state.approvalNotificationChannelPolicies = approvalNotificationChannelPolicies;
  state.approvalNotificationRouting = approvalNotificationRouting;
  state.approvalNotificationRuns = approvalNotificationRuns;
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
    const [teams, memberships, tenantInvitations] = await Promise.all([
      api(`/api/organizations/${state.selectedOrganizationId}/teams`),
      api(`/api/organizations/${state.selectedOrganizationId}/memberships`),
      api(`/api/organizations/${state.selectedOrganizationId}/invitations`),
    ]);
    state.teams = teams;
    state.memberships = memberships;
    state.tenantInvitations = tenantInvitations;
    if (state.selectedTeamId && !teams.some((team) => team.id === state.selectedTeamId)) {
      state.selectedTeamId = "";
    }
    if (!state.selectedTeamId && teams[0]) {
      setTeamId(teams[0].id);
    }
  } else {
    state.teams = [];
    state.memberships = [];
    state.tenantInvitations = [];
    state.selectedTeamId = "";
  }
  state.projects = state.selectedTeamId
    ? await api(`/api/teams/${state.selectedTeamId}/projects`)
    : [];
  renderOps();
}

async function refreshAgentReleases(render = true) {
  const [summary, automationRuns, releaseEntries] = await Promise.all([
    api("/api/agents/releases/summary"),
    api("/api/agents/releases/automation-runs"),
    Promise.all(
      state.agents.map(async (agent) => [agent.id, await api(`/api/agents/${agent.id}/releases`)]),
    ),
  ]);
  state.agentReleaseSummary = summary;
  state.agentReleaseAutomationRuns = automationRuns;
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
  await refreshOps();
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

async function runApprovalNotifications() {
  state.approvalNotificationRun = await api("/api/approvals/notifications/run", {
    method: "POST",
  });
  state.approvalNotificationRuns = await api("/api/approvals/notifications/runs");
  state.approvalNotificationRouting = await api("/api/approvals/notification-routing/summary");
  renderApprovalNotificationRouting();
  renderApprovalNotificationRuns();
  await refreshApprovals();
}

async function validateApprovalNotifications() {
  state.approvalNotificationDeploymentValidation = await api(
    "/api/approvals/notifications/deployment/validate",
    {
      method: "POST",
    },
  );
  state.approvalNotificationRuns = await api("/api/approvals/notifications/runs");
  state.approvalNotificationRouting = await api("/api/approvals/notification-routing/summary");
  renderApprovalNotificationRouting();
  renderApprovalNotificationRuns();
}

function renderOps() {
  renderUsage();
  renderObservability();
  renderTenantGovernance();
  renderProviders();
  renderVaultHealth();
  renderVaultReadiness();
  renderSecretRecords();
  renderApprovalGovernance();
  renderApprovalNotificationRuns();
  renderPolicy();
  renderEvalJudgeProfiles();
  renderEvalSuiteBootstrap();
  renderEvalDatasets();
  renderEvalCases();
  renderEvalRuns();
  renderAgentReleases();
  renderMcpServers();
  renderWorkerReadiness();
  renderRemoteComputerReadiness();
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
  const runSummary = summarizeCodexRuns(codex.runs || []);
  const responseCards = [
    ["Health", codex.health],
    ["Deployment Validation", codex.deploymentValidation],
    ["Thread", codex.thread],
    ["Turn", codex.turn],
    ["Command", codex.command],
    ["Interrupt", codex.interrupt],
    ["Poll", codex.poll],
    ["Stale Poll", codex.stalePoll],
    ["Trace Detail", codex.traceDetail],
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
  const traceSummary = codex.traces;
  const controlSummary = codex.controlSummary;
  const productionOps = controlSummary?.production_ops || {};
  const deploymentReadiness = controlSummary?.deployment_readiness || {};
  const controlAttention = controlSummary?.attention_items || [];
  const traceDetail = codex.traceDetail;
  const traceRows = (traceSummary?.traces || [])
    .slice(0, 8)
    .map(
      (trace) => `
        <tr>
          <td>${escapeHtml(trace.turn_id || trace.trace_key)}</td>
          <td>${escapeHtml(trace.latest_status)}</td>
          <td>${formatInteger(trace.run_count)} runs · ${formatInteger(trace.command_count)} commands · ${formatInteger(trace.poll_count)} polls</td>
          <td>${formatInteger(trace.error_count)}</td>
          <td>${escapeHtml(trace.next_action || "none")}</td>
          <td>${formatDurationSeconds(trace.duration_seconds)}</td>
          <td>${escapeHtml((trace.operations || []).join(", "))}</td>
          <td>${escapeHtml(trace.last_seen_at)}</td>
          <td><button type="button" class="secondary" data-codex-trace="${escapeHtml(trace.trace_key)}">Detail</button></td>
        </tr>
      `,
    )
    .join("");
  const traceDashboard = traceSummary
    ? `<h4>Per-turn Trace Summary</h4>
      <div class="metric-grid compact-metrics">
        <div class="metric"><span>Runs</span><strong>${formatInteger(traceSummary.run_count)}</strong></div>
        <div class="metric"><span>Turns</span><strong>${formatInteger(traceSummary.turn_count)}</strong></div>
        <div class="metric"><span>Active</span><strong>${formatInteger(traceSummary.active_turn_count)}</strong></div>
        <div class="metric"><span>Failed</span><strong>${formatInteger(traceSummary.failed_turn_count)}</strong></div>
      </div>
      ${
        traceRows
          ? `<table class="usage-table">
              <thead>
                <tr>
                  <th>Turn</th>
                  <th>Status</th>
                  <th>Activity</th>
                  <th>Errors</th>
                  <th>Next</th>
                  <th>Duration</th>
                  <th>Operations</th>
                  <th>Last Seen</th>
                  <th>Detail</th>
                </tr>
              </thead>
              <tbody>${traceRows}</tbody>
            </table>`
          : `<div class="muted">No Codex turn traces yet.</div>`
      }`
    : "";
  const traceDetailDashboard = traceDetail
    ? `<h4>Trace Detail</h4>
      <div class="item">
        <strong>${escapeHtml(traceDetail.trace?.turn_id || traceDetail.trace?.trace_key || "Codex trace")}</strong>
        <div class="muted">Status ${escapeHtml(traceDetail.trace?.latest_status || "unknown")} · next ${escapeHtml(traceDetail.trace?.next_action || "none")} · duration ${formatDurationSeconds(traceDetail.trace?.duration_seconds || 0)} · terminal ${traceDetail.trace?.terminal ? "yes" : "no"}</div>
        <div class="muted">Commands ${escapeHtml((traceDetail.command_ids || traceDetail.trace?.command_ids || []).join(", ") || "none")} · terminal runs ${formatInteger(traceDetail.terminal_count || 0)} · non-terminal runs ${formatInteger(traceDetail.non_terminal_count || 0)}</div>
        ${
          traceDetail.trace?.latest_error
            ? `<pre>${escapeHtml(JSON.stringify(traceDetail.trace.latest_error, null, 2))}</pre>`
            : `<div class="muted">No latest error for this trace.</div>`
        }
      </div>
      <div class="metric-grid compact-metrics">
        ${Object.entries(traceDetail.by_status || {})
          .map(
            ([status, count]) =>
              `<div class="metric"><span>${escapeHtml(status)}</span><strong>${formatInteger(count)}</strong></div>`,
          )
          .join("")}
      </div>
      ${
        (traceDetail.status_timeline || []).length
          ? `<table class="usage-table">
              <thead>
                <tr>
                  <th>Operation</th>
                  <th>Status</th>
                  <th>Terminal</th>
                  <th>Created</th>
                  <th>Error</th>
                </tr>
              </thead>
              <tbody>
                ${(traceDetail.status_timeline || [])
                  .map(
                    (point) => `
                      <tr>
                        <td>${escapeHtml(point.operation)}</td>
                        <td>${escapeHtml(point.status)}</td>
                        <td>${point.terminal ? "yes" : "no"}</td>
                        <td>${escapeHtml(point.created_at)}</td>
                        <td>${escapeHtml(point.error ? JSON.stringify(point.error) : "none")}</td>
                      </tr>
                    `,
                  )
                  .join("")}
              </tbody>
            </table>`
          : `<div class="muted">No trace timeline points.</div>`
      }`
    : "";
  const runDashboard = `
      <h4>Long-running Steering</h4>
      <div class="metric-grid compact-metrics">
        <div class="metric"><span>Total Runs</span><strong>${formatInteger(runSummary.total)}</strong></div>
        <div class="metric"><span>Active Turns</span><strong>${formatInteger(runSummary.active)}</strong></div>
        <div class="metric"><span>Terminal Runs</span><strong>${formatInteger(runSummary.terminal)}</strong></div>
        <div class="metric"><span>Failed Runs</span><strong>${formatInteger(runSummary.failed)}</strong></div>
      </div>
      <div class="item">
        <strong>Pollable turns</strong>
        <div class="muted">${escapeHtml(runSummary.pollableLabels.join(", ") || "none")}</div>
      </div>
    `;
  const controlDashboard = controlSummary
    ? `<h4>Control-plane Summary</h4>
      <div class="metric-grid compact-metrics">
        <div class="metric"><span>Status</span><strong>${escapeHtml(controlSummary.status)}</strong></div>
        <div class="metric"><span>Runs</span><strong>${formatInteger(controlSummary.run_count)}</strong></div>
        <div class="metric"><span>Turns</span><strong>${formatInteger(controlSummary.turn_count)}</strong></div>
        <div class="metric"><span>Active</span><strong>${formatInteger(controlSummary.active_turn_count)}</strong></div>
        <div class="metric"><span>Failed</span><strong>${formatInteger(controlSummary.failed_turn_count)}</strong></div>
        <div class="metric"><span>Stale</span><strong>${formatInteger(controlSummary.stale_candidate_count)}</strong></div>
        <div class="metric"><span>Pollable</span><strong>${formatInteger(controlSummary.pollable_turn_count)}</strong></div>
        <div class="metric"><span>Attention</span><strong>${formatInteger(controlAttention.length)}</strong></div>
      </div>
      <div class="muted">Configured: ${escapeHtml(controlSummary.configured ? "yes" : "no")} · timeout ${escapeHtml(controlSummary.timeout_seconds ?? "n/a")}s · latest ${escapeHtml(controlSummary.latest_seen_at || "none")}</div>
      <div class="muted">Production ops: ${escapeHtml(productionOps.status || "unknown")} · blocked ${productionOps.production_blocked ? "yes" : "no"} · stale candidates ${formatInteger(productionOps.stale_candidate_count || 0)} · failed turns ${formatInteger(productionOps.failed_turn_count || 0)} · latest supervision ${escapeHtml(productionOps.latest_stale_poll_at || "none")}</div>
      <div class="muted">${escapeHtml(productionOps.message || "Codex App Server production ops are not reported")}</div>
      <div class="muted">Deployment validation: ${escapeHtml(deploymentReadiness.status || "unknown")} · blocked ${deploymentReadiness.production_blocked ? "yes" : "no"} · validated ${deploymentReadiness.deployment_validated ? "yes" : "no"} · healthy ${deploymentReadiness.latest_validation_healthy ? "yes" : "no"} · latest ${escapeHtml(deploymentReadiness.latest_validation_at || "none")}</div>
      <div class="muted">${escapeHtml(deploymentReadiness.message || "Codex App Server deployment validation is not reported")}</div>
      ${
        controlAttention.length
          ? `<table class="usage-table">
              <thead>
                <tr>
                  <th>Severity</th>
                  <th>Signal</th>
                  <th>Turn</th>
                  <th>Message</th>
                </tr>
              </thead>
              <tbody>
                ${controlAttention
                  .map(
                    (item) => `
                      <tr>
                        <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                        <td>${escapeHtml(item.kind)}</td>
                        <td>${escapeHtml(item.turn_id || item.trace_key || "global")}</td>
                        <td>${escapeHtml(item.message)}</td>
                      </tr>
                    `,
                  )
                  .join("")}
              </tbody>
            </table>`
          : `<div class="muted">No Codex App Server attention items.</div>`
      }`
    : "";
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
    ${controlDashboard}
    ${runDashboard}
    ${traceDashboard}
    ${traceDetailDashboard}
    ${runs ? `<h4>Persisted Codex Runs</h4>${runs}` : ""}
  `;
  codexAppServerRoot.querySelectorAll("[data-poll-codex-run]").forEach((button) => {
    button.addEventListener("click", () => pollCodexRun(button.dataset.pollCodexRun));
  });
  codexAppServerRoot.querySelectorAll("[data-codex-trace]").forEach((button) => {
    button.addEventListener("click", () => loadCodexTraceDetail(button.dataset.codexTrace));
  });
}

function summarizeCodexRuns(runs) {
  const terminalStatuses = new Set(["completed", "failed", "cancelled", "canceled", "interrupted"]);
  const failedStatuses = new Set(["failed", "poll_failed", "cancelled", "canceled", "interrupted"]);
  const summary = {
    total: runs.length,
    active: 0,
    terminal: 0,
    failed: 0,
    pollableLabels: [],
  };
  runs.forEach((run) => {
    const status = String(run.status || "unknown").toLowerCase();
    const terminal = terminalStatuses.has(status);
    if (terminal) summary.terminal += 1;
    if (!terminal && run.turn_id) summary.active += 1;
    if (failedStatuses.has(status)) summary.failed += 1;
    if (run.turn_id && !terminal) {
      summary.pollableLabels.push(`${run.operation}:${run.turn_id}`);
    }
  });
  return summary;
}

function renderWorkerReadiness() {
  const report = state.workerReadiness;
  if (!report) {
    workerReadinessRoot.innerHTML = `<div class="muted">Worker readiness not loaded</div>`;
    return;
  }
  const jobSummary = report.job_summary || {};
  const leaseSummary = report.lease_summary || {};
  const queueBackend = report.queue_backend || {};
  const workerMode = report.worker_mode || {};
  const k8s = report.k8s || {};
  const autoscaling = report.autoscaling || {};
  const loadValidation = report.load_validation || {};
  const productionOps = report.production_ops || {};
  const loadValidationRun = state.workerLoadValidationRun;
  const attentionItems = report.attention_items || [];
  const runbookActions = report.runbook_actions || [];
  workerReadinessRoot.innerHTML = `
    <div class="metrics compact-metrics">
      <div class="metric">
        <span>Status</span>
        <strong>${escapeHtml(report.status || "unknown")}</strong>
      </div>
      <div class="metric">
        <span>Score</span>
        <strong>${formatInteger(report.readiness_score || 0)}</strong>
      </div>
      <div class="metric">
        <span>Queue Backend</span>
        <strong>${escapeHtml(queueBackend.kind || "unknown")}</strong>
      </div>
      <div class="metric">
        <span>Worker Mode</span>
        <strong>${escapeHtml(workerMode.mode || "unknown")}</strong>
      </div>
      <div class="metric">
        <span>Prod Ops</span>
        <strong>${escapeHtml(productionOps.status || "unknown")}</strong>
      </div>
    </div>
    <div class="item">
      <strong>PRODUCTION OPS GATE</strong>
      <div class="muted">${escapeHtml(productionOps.message || "worker production ops gate is not reported")}</div>
      <div class="muted">durable queue ${productionOps.durable_queue ? "yes" : "no"} · queue worker ${productionOps.queue_worker_mode ? "yes" : "no"} · hardened Pod ${productionOps.hardened_worker_pod ? "yes" : "no"} · queue-depth autoscaling ${productionOps.queue_depth_autoscaling ? "yes" : "no"} · isolated pool ${productionOps.isolated_worker_pool_configured ? "yes" : "no"} · load validated ${productionOps.load_validated ? "yes" : "no"}</div>
    </div>
    <div class="item">
      <strong>QUEUE DURABILITY</strong>
      <div class="muted">${escapeHtml(queueBackend.semantics || "not reported")}</div>
      <div class="muted">Durable: ${queueBackend.durable ? "yes" : "no"} · Broker handoff: ${queueBackend.broker_handoff ? "yes" : "no"} · JetStream: ${queueBackend.jetstream_enabled ? "yes" : "no"}</div>
    </div>
    <div class="item">
      <strong>JOB PRESSURE</strong>
      <div class="muted">Queued ${formatInteger(jobSummary.queued_jobs || 0)} · Running ${formatInteger(jobSummary.running_jobs || 0)} · Retryable ${formatInteger(jobSummary.retryable_jobs || 0)} · Failed ${formatInteger(jobSummary.failed_jobs || 0)}</div>
      <div class="muted">Oldest queued age: ${formatOptionalSeconds(jobSummary.oldest_queued_job_age_seconds)}</div>
    </div>
    <div class="item">
      <strong>LEASES / AUTOSCALING</strong>
      <div class="muted">Leased ${formatInteger(leaseSummary.leased_jobs || 0)} · Stale leases ${formatInteger(leaseSummary.stale_leases || 0)} · Oldest stale lease ${formatOptionalSeconds(leaseSummary.oldest_stale_lease_age_seconds)}</div>
      <div class="muted">K8s worker manifest: ${k8s.worker_manifest_present ? "present" : "missing"} · Autoscaling manifest: ${autoscaling.autoscaling_manifest_present ? "present" : "missing"}</div>
      <div class="muted">WORKER HARDENING: ${escapeHtml(k8s.hardening_status || "unknown")} · SA ${escapeHtml(k8s.service_account_name || "none")} · NetworkPolicy ${k8s.network_policy_present ? "present" : "missing"}</div>
      <div class="muted">Security: non-root ${k8s.pod_run_as_non_root ? "yes" : "no"} · seccomp ${k8s.seccomp_runtime_default ? "RuntimeDefault" : "missing"} · no privilege escalation ${k8s.container_allow_privilege_escalation_disabled ? "yes" : "no"} · drop ALL caps ${k8s.container_drops_all_capabilities ? "yes" : "no"} · read-only root ${k8s.container_read_only_root_filesystem ? "yes" : "no"}</div>
      <div class="muted">Resources: requests ${k8s.resources_requests_configured ? "configured" : "missing"} · limits ${k8s.resources_limits_configured ? "configured" : "missing"} · token automount ${k8s.automount_service_account_token_disabled ? "disabled" : "enabled/unknown"}</div>
      <div class="muted">AUTOSCALING SKELETON: ${escapeHtml(autoscaling.validation_status || "unknown")} · min ${formatOptionalInteger(autoscaling.configured_min_replicas)} · max ${formatOptionalInteger(autoscaling.configured_max_replicas)}</div>
      <div class="muted">Targets: ${escapeHtml((autoscaling.scale_target_refs || []).join(", ") || "none")}</div>
    </div>
    <div class="item">
      <strong>WORKER LOAD VALIDATION</strong>
      <div class="muted">${escapeHtml(loadValidation.status || "unknown")} · latest ${escapeHtml(loadValidation.latest_run_status || "none")} · isolated pool ${loadValidation.isolated_worker_pool_configured ? "configured" : "missing"} · load validated ${loadValidation.load_validated ? "yes" : "no"}</div>
      <div class="muted">${escapeHtml(loadValidation.message || "Load validation has not been reported")}</div>
      <div class="muted">Required: ${escapeHtml(loadValidation.required_profile || "not reported")}</div>
      ${
        loadValidationRun
          ? `<pre>${escapeHtml(JSON.stringify(loadValidationRun, null, 2))}</pre>`
          : `<div class="muted">No worker load validation run in this console session</div>`
      }
    </div>
    <div class="item">
      <strong>ATTENTION ITEMS</strong>
      ${
        attentionItems.length
          ? attentionItems
              .map(
                (item) =>
                  `<div class="muted">${escapeHtml(item.severity)} · ${escapeHtml(item.kind)} · ${escapeHtml(item.message)}</div>`,
              )
              .join("")
          : `<div class="muted">No worker readiness attention items</div>`
      }
    </div>
    <div class="item">
      <strong>WORKER RUNBOOK ACTIONS</strong>
      ${
        runbookActions.length
          ? runbookActions.map((action) => `<div class="muted">${escapeHtml(action)}</div>`).join("")
          : `<div class="muted">No worker runbook actions</div>`
      }
    </div>
  `;
}

function renderRemoteComputerReadiness() {
  const report = state.remoteComputerReadiness;
  if (!report) {
    remoteComputerReadinessRoot.innerHTML = `<div class="muted">Remote computer readiness not loaded</div>`;
    return;
  }
  const podTemplate = report.pod_template || {};
  const serviceAccount = report.service_account || {};
  const stateFs = report.state_filesystem || {};
  const productionStateSync = report.production_state_sync || {};
  const networkPolicy = report.network_policy || {};
  const autoscaling = report.autoscaling || {};
  const warmPool = report.warm_pool || {};
  const artifactDiscoverySidecar = report.artifact_discovery_sidecar || {};
  const sidecarSupervision = report.sidecar_supervision || {};
  const sidecarRecovery = report.sidecar_recovery || {};
  const runner = state.remoteComputerRunnerReadiness || report.runner || {};
  const executionTransport = report.execution_transport || {};
  const attentionItems = report.attention_items || [];
  const runbookActions = report.runbook_actions || [];
  const computerRows = state.remoteComputers || [];
  const leaseRows = state.remoteComputerLeases || [];
  const attachmentRows = state.remoteComputerAttachments || [];
  const assignmentRows = state.remoteComputerJobAssignments || [];
  const stateLockRows = state.remoteComputerStateLocks || [];
  const sidecarHeartbeatRows = state.remoteComputerSidecarHeartbeats || [];
  const artifactDiscovery = state.remoteComputerArtifactDiscovery;
  const sidecarRecoveryRun = state.remoteComputerSidecarRecoveryRun;
  remoteComputerReadinessRoot.innerHTML = `
    <div class="metrics compact-metrics">
      <div class="metric">
        <span>Status</span>
        <strong>${escapeHtml(report.status || "unknown")}</strong>
      </div>
      <div class="metric">
        <span>Score</span>
        <strong>${formatInteger(report.readiness_score || 0)}</strong>
      </div>
      <div class="metric">
        <span>Pod Template</span>
        <strong>${podTemplate.present ? "present" : "missing"}</strong>
      </div>
      <div class="metric">
        <span>State FS</span>
        <strong>${escapeHtml(stateFs.status || "unknown")}</strong>
      </div>
      <div class="metric">
        <span>Prod State Sync</span>
        <strong>${escapeHtml(productionStateSync.status || "unknown")}</strong>
      </div>
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER READINESS</strong>
      <div class="muted">Pod: ${escapeHtml(podTemplate.path || "unknown")} · ${podTemplate.present ? "present" : "missing"}</div>
      <div class="muted">Service account: ${escapeHtml(serviceAccount.path || "unknown")} · ${serviceAccount.present ? "present" : "missing"}</div>
      <div class="muted">NetworkPolicy: ${escapeHtml(networkPolicy.path || "unknown")} · ${networkPolicy.present ? "present" : "missing"}</div>
    </div>
    <div class="item">
      <strong>STATE FILESYSTEM</strong>
      <div class="muted">${escapeHtml(stateFs.provider || "unknown")} · ${escapeHtml(stateFs.access_mode || "unknown")} · ${escapeHtml(stateFs.mount_path || "unknown")}</div>
      <div class="muted">PVC: ${escapeHtml(stateFs.pvc_path || "unknown")} · ${stateFs.pvc_present ? "present" : "missing"} · distributed state: ${stateFs.distributed_filesystem_configured ? "configured" : "not configured"}</div>
      <div class="muted">Production profile: ${escapeHtml(stateFs.production_profile_path || "unknown")} · ${stateFs.production_profile_present ? "present" : "missing"} · claim ${escapeHtml(stateFs.production_claim_name || "unknown")}</div>
      <div class="muted">Contract: ${escapeHtml(stateFs.state_contract_path || "unknown")} · ${stateFs.state_contract_present ? "present" : "missing"} · conflict policy ${escapeHtml(stateFs.conflict_policy || "unknown")} · lock manager ${stateFs.lock_manager_configured ? "configured" : "missing"}</div>
      <div class="muted">Layout: ${escapeHtml((stateFs.state_layout_paths || []).join(", ") || "not reported")} · sync contract ${escapeHtml(stateFs.sync_contract_status || "unknown")}</div>
      <div class="muted">Provider source: ${stateFs.provider_configured_by_env ? "env" : "placeholder"} · example manifest: ${stateFs.provider_manifest_present ? "present" : "missing"} · ${escapeHtml((stateFs.supported_providers || []).join(", ") || "no providers listed")}</div>
    </div>
    <div class="item">
      <strong>PRODUCTION STATE SYNC GATE</strong>
      <div class="muted">${escapeHtml(productionStateSync.message || "Remote Computer production state sync gate is not reported")}</div>
      <div class="muted">provider ${escapeHtml(productionStateSync.provider || "unknown")} · distributed ${productionStateSync.distributed_filesystem_configured ? "yes" : "no"} · profile ${productionStateSync.production_profile_present ? "present" : "missing"} · contract ${productionStateSync.state_contract_present ? "present" : "missing"} · lock manager ${productionStateSync.lock_manager_configured ? "yes" : "no"}</div>
    </div>
    <div class="item">
      <strong>WARM POOL / SCALING</strong>
      <div class="muted">Warm pool: ${escapeHtml(warmPool.status || "unknown")} · manifest ${warmPool.manifest_present ? "present" : "missing"}</div>
      <div class="muted">Artifact discovery sidecar: ${escapeHtml(artifactDiscoverySidecar.status || "unknown")} · manifest ${artifactDiscoverySidecar.present ? "present" : "missing"} · ${escapeHtml(artifactDiscoverySidecar.path || "unknown")}</div>
      <div class="muted">Worker HPA ${autoscaling.worker_hpa_present ? "present" : "missing"} · KEDA ${autoscaling.keda_manifest_present ? "present" : "missing"} · remote pool scaler ${autoscaling.remote_pool_scaled_object_present ? "present" : "missing"} · queue-depth scaling ${autoscaling.queue_depth_scaling_present ? "present" : "missing"}</div>
    </div>
    <div class="item">
      <strong>RUNNER BOUNDARY</strong>
      <div class="muted">${escapeHtml(runner.status || "unknown")} · mode ${escapeHtml(runner.mode || "reserved")} · configured ${runner.configured ? "yes" : "no"}</div>
      <div class="muted">Client configured: ${runner.client_configured ? "yes" : "no"} · mutation gate: ${runner.mutation_enabled ? "yes" : "no"} · live mutation gate: ${runner.live_mutation_enabled ? "yes" : "no"} · dry-run only: ${runner.dry_run_only === false ? "no" : "yes"}</div>
      <div class="muted">API server: ${runner.api_server_configured ? "configured" : "not configured"} · bearer token: ${runner.bearer_token_configured ? "configured" : "not configured"}</div>
      <div class="muted">Namespace: ${escapeHtml(runner.namespace || "unknown")} · Service account: ${escapeHtml(runner.service_account || "unknown")}</div>
      <div class="muted">${escapeHtml(runner.message || "Kubernetes Pod mutation is disabled unless a runner is explicitly implemented")}</div>
    </div>
    <div class="item">
      <strong>EXECUTION TRANSPORT</strong>
      <div class="muted">${escapeHtml(executionTransport.status || "unknown")} · mode ${escapeHtml(executionTransport.mode || "reserved")} · requested ${executionTransport.requested_execution_enabled ? "yes" : "no"} · enabled ${executionTransport.execution_enabled ? "yes" : "no"}</div>
      <div class="muted">Assignments ${formatInteger(executionTransport.assignment_count || 0)} · active ${formatInteger(executionTransport.active_assignment_count || 0)}</div>
      <div class="muted">Required: ${escapeHtml((executionTransport.required_implementation || []).join(", ") || "not reported")}</div>
      <div class="muted">${escapeHtml(executionTransport.message || "Pod exec transport is not implemented")}</div>
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER EVENTS</strong>
      <div class="muted">${escapeHtml((report.event_types || []).join(", ") || "none")}</div>
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER LEASE STORE</strong>
      <div class="muted">${formatInteger(computerRows.length)} computers · ${formatInteger(leaseRows.length)} leases · ${formatInteger(attachmentRows.length)} attachments · ${formatInteger(assignmentRows.length)} job handoffs · ${formatInteger(stateLockRows.length)} state locks · execution remains on approved worker path</div>
      ${
        computerRows.length
          ? computerRows
              .map(
                (computer) =>
                  `<div class="muted">${escapeHtml(computer.name)} · ${escapeHtml(computer.profile)} · ${escapeHtml(computer.status)} · ${escapeHtml(computer.pod_name || "no pod")}</div>`,
              )
              .join("")
          : `<div class="muted">No remote computers registered</div>`
      }
      ${
        leaseRows.length
          ? leaseRows
              .map(
                (lease) =>
                  `<div class="muted">lease ${escapeHtml(lease.id)} · ${escapeHtml(lease.status)} · session ${escapeHtml(lease.session_id || "none")}</div>`,
              )
              .join("")
          : `<div class="muted">No remote computer leases</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER ATTACHMENTS</strong>
      ${
        attachmentRows.length
          ? attachmentRows
              .map(
                (attachment) =>
                  `<div class="muted">${escapeHtml(attachment.status)} · session ${escapeHtml(attachment.session_id)} · lease ${escapeHtml(attachment.lease_id)} · stale after ${escapeHtml(attachment.stale_after || "none")}</div>`,
              )
              .join("")
          : `<div class="muted">No remote computer session attachments</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER JOB HANDOFFS</strong>
      ${
        assignmentRows.length
          ? assignmentRows
              .map(
                (assignment) =>
                  `<div class="muted">${escapeHtml(assignment.status)} · job ${escapeHtml(assignment.execution_job_id)} · lease ${escapeHtml(assignment.lease_id)} · session ${escapeHtml(assignment.session_id)}</div>`,
              )
              .join("")
          : `<div class="muted">No remote computer job handoffs</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER STATE LOCKS</strong>
      <div class="muted">Acquire State Lock coordinates Memory / Notes / Skills / Artifacts writes before multi-Pod shared-state mutation.</div>
      ${
        stateLockRows.length
          ? stateLockRows
              .map(
                (lock) =>
                  `<div class="muted">${escapeHtml(lock.status)} · ${escapeHtml(lock.lock_key)} · owner ${escapeHtml(lock.owner || "none")} · expires ${escapeHtml(lock.expires_at || "none")} ${
                    lock.status === "held"
                      ? `<button class="secondary inline-button" data-release-remote-state-lock="${escapeHtml(lock.id)}">Release State Lock</button>`
                      : ""
                  }</div>`,
              )
              .join("")
          : `<div class="muted">No remote computer state locks</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE ARTIFACT DISCOVERY</strong>
      <div class="muted">Discover Remote Artifacts scans a shared Remote Computer workspace and records artifacts, events, and audit logs.</div>
      ${
        artifactDiscovery
          ? artifactDiscovery.error
            ? `<div class="muted">Discovery error: ${escapeHtml(artifactDiscovery.error)}</div>`
            : `<div class="muted">Last discovery: ${formatInteger(artifactDiscovery.artifact_count || 0)} artifacts from ${escapeHtml(artifactDiscovery.remote_computer_id || "unknown")}</div>`
          : `<div class="muted">No remote artifact discovery run in this console session</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER SIDECAR HEARTBEATS</strong>
      <div class="muted">Artifact discovery sidecars post heartbeat events so operators can distinguish manifest presence from live sidecar activity.</div>
      <div class="muted">Supervision: ${escapeHtml(sidecarSupervision.status || "unknown")} · active computers ${formatInteger(sidecarSupervision.active_remote_computer_count || 0)} · total heartbeats ${formatInteger(sidecarSupervision.heartbeat_count || 0)} · missing ${formatInteger(sidecarSupervision.missing_heartbeat_count || 0)} · stale ${formatInteger(sidecarSupervision.stale_heartbeat_count || 0)} · stale after ${formatInteger(sidecarSupervision.stale_after_seconds || 0)}s</div>
      <div class="muted">Latest observed: ${escapeHtml(sidecarSupervision.latest_observed_at || "none")}</div>
      ${
        sidecarHeartbeatRows.length
          ? sidecarHeartbeatRows
              .slice(0, 10)
              .map(
                (heartbeat) =>
                  `<div class="muted">${escapeHtml(heartbeat.sidecar_name)} · ${escapeHtml(heartbeat.status)} · remote ${escapeHtml(heartbeat.remote_computer_id)} · observed ${escapeHtml(heartbeat.observed_at)}</div>`,
              )
              .join("")
          : `<div class="muted">No remote computer sidecar heartbeats</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER SIDECAR RECOVERY</strong>
      <div class="muted">${escapeHtml(sidecarRecovery.status || "unknown")} · replacement gate ${sidecarRecovery.replacement_enabled ? "enabled" : "disabled"} · runner ${sidecarRecovery.runner_configured ? "configured" : "not configured"} · live mutation ${sidecarRecovery.runner_live_mutation_enabled ? "enabled" : "disabled"}</div>
      <div class="muted">Unhealthy ${formatInteger(sidecarRecovery.unhealthy_count || 0)} · replaceable Pods ${formatInteger(sidecarRecovery.replaceable_pod_count || 0)} · blocked ${escapeHtml(sidecarRecovery.blocked_reason || "none")}</div>
      <div class="muted">${escapeHtml(sidecarRecovery.message || "Sidecar recovery gate is not reported")}</div>
      <button class="secondary inline-button" data-run-remote-sidecar-recovery="true">Run Sidecar Recovery Gate</button>
      ${
        sidecarRecoveryRun
          ? `<div class="muted">Last run: ${escapeHtml(sidecarRecoveryRun.status || "unknown")} · unhealthy ${formatInteger(sidecarRecoveryRun.unhealthy_count || 0)} · attempted ${formatInteger(sidecarRecoveryRun.attempted_replacement_count || 0)} · blocked ${formatInteger(sidecarRecoveryRun.blocked_replacement_count || 0)} · validation ${escapeHtml(sidecarRecoveryRun.validation_result?.status || "unknown")} · ${escapeHtml(sidecarRecoveryRun.message || "")}</div>`
          : `<div class="muted">No sidecar recovery run in this console session</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER ATTENTION</strong>
      ${
        attentionItems.length
          ? attentionItems
              .map(
                (item) =>
                  `<div class="muted">${escapeHtml(item.severity)} · ${escapeHtml(item.kind)} · ${escapeHtml(item.message)}</div>`,
              )
              .join("")
          : `<div class="muted">No remote computer attention items</div>`
      }
    </div>
    <div class="item">
      <strong>REMOTE COMPUTER RUNBOOK</strong>
      ${
        runbookActions.length
          ? runbookActions.map((action) => `<div class="muted">${escapeHtml(action)}</div>`).join("")
          : `<div class="muted">No remote computer runbook actions</div>`
      }
    </div>
  `;
  remoteComputerReadinessRoot.querySelectorAll("[data-release-remote-state-lock]").forEach((button) => {
    button.addEventListener("click", () => releaseRemoteStateLock(button.dataset.releaseRemoteStateLock));
  });
  remoteComputerReadinessRoot.querySelectorAll("[data-run-remote-sidecar-recovery]").forEach((button) => {
    button.addEventListener("click", runRemoteSidecarRecovery);
  });
}

function formatOptionalSeconds(value) {
  if (value === null || value === undefined) return "none";
  return `${formatInteger(value)}s`;
}

function optionalFormValue(form, name) {
  const value = String(form.get(name) || "").trim();
  return value || undefined;
}

async function discoverRemoteArtifacts(event) {
  event.preventDefault();
  const form = new FormData(remoteArtifactDiscoveryForm);
  const sessionId = optionalFormValue(form, "session_id");
  const remoteComputerId = optionalFormValue(form, "remote_computer_id");
  if (!sessionId || !remoteComputerId) {
    state.remoteComputerArtifactDiscovery = {
      error: "Session ID and Remote computer ID are required before artifact discovery",
    };
    renderRemoteComputerReadiness();
    return;
  }
  state.remoteComputerArtifactDiscovery = await api("/api/remote-computers/artifacts/discover", {
    method: "POST",
    body: JSON.stringify({
      session_id: sessionId,
      remote_computer_id: remoteComputerId,
      artifact_dir: optionalFormValue(form, "artifact_dir") || "artifacts",
      max_files: 10,
    }),
  });
  await refreshOps();
}

async function acquireRemoteStateLock(event) {
  event.preventDefault();
  const form = new FormData(remoteStateLockForm);
  await api("/api/remote-computers/state-locks", {
    method: "POST",
    body: JSON.stringify({
      lock_key: optionalFormValue(form, "lock_key") || "memory/session-notes.md",
      session_id: optionalFormValue(form, "session_id"),
      remote_computer_id: optionalFormValue(form, "remote_computer_id"),
      lease_id: optionalFormValue(form, "lease_id"),
      owner: "static-admin-console",
      lease_seconds: 900,
    }),
  });
  await refreshOps();
}

async function releaseRemoteStateLock(lockId) {
  await api(`/api/remote-computers/state-locks/${lockId}/release`, {
    method: "POST",
    body: JSON.stringify({ reason: "released-from-static-admin-console" }),
  });
  await refreshOps();
}

async function runRemoteSidecarRecovery() {
  state.remoteComputerSidecarRecoveryRun = await api("/api/remote-computers/sidecars/recovery/run", {
    method: "POST",
  });
  await refreshOps();
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
  renderTenantIsolationReadiness();
  const provisioning = state.tenantProvisioning
    ? `<div class="item">
        <strong>Tenant provisioned</strong>
        <div class="muted">Org ${escapeHtml(state.tenantProvisioning.organization.id)} · Team ${escapeHtml(state.tenantProvisioning.team?.id || "none")} · Owner ${escapeHtml(state.tenantProvisioning.owner_membership.user_id)}</div>
      </div>`
    : "";
  organizationRoot.innerHTML = state.organizations.length
    ? `${provisioning}${state.organizations
        .map(
          (organization) => `
            <div class="item">
              <button class="item-button${organization.id === state.selectedOrganizationId ? " selected" : ""}" data-organization="${organization.id}">
                <strong>${escapeHtml(organization.name)}</strong>
                <span>${escapeHtml(organization.slug)} · ${escapeHtml(organization.id)} · ${escapeHtml(organization.archived_at ? "archived" : "active")}</span>
                <span>Owner: ${escapeHtml(organization.owner_subject || "unassigned")}</span>
              </button>
              ${
                organization.archived_at
                  ? `<button type="button" class="secondary" data-delete-organization="${escapeHtml(organization.id)}">Delete Organization</button>`
                  : `<button type="button" class="secondary" data-archive-organization="${escapeHtml(organization.id)}">Archive Organization</button>`
              }
            </div>
          `,
        )
        .join("")}`
    : `${provisioning}<div class="muted">No organizations yet</div>`;
  organizationRoot.querySelectorAll("[data-organization]").forEach((button) => {
    button.addEventListener("click", async () => {
      setOrganizationId(button.dataset.organization);
      state.selectedTeamId = "";
      await refreshOps();
    });
  });
  organizationRoot.querySelectorAll("[data-archive-organization]").forEach((button) => {
    button.addEventListener("click", () => archiveOrganization(button.dataset.archiveOrganization));
  });
  organizationRoot.querySelectorAll("[data-delete-organization]").forEach((button) => {
    button.addEventListener("click", () => deleteOrganization(button.dataset.deleteOrganization));
  });

  teamRoot.innerHTML = state.selectedOrganizationId
    ? state.teams.length
      ? state.teams
          .map(
            (team) => `
              <div class="item">
                <button class="item-button${team.id === state.selectedTeamId ? " selected" : ""}" data-team="${team.id}">
                  <strong>${escapeHtml(team.name)}</strong>
                  <span>${escapeHtml(team.slug)} · ${escapeHtml(team.id)} · ${escapeHtml(team.archived_at ? "archived" : "active")}</span>
                </button>
                ${
                  team.archived_at
                    ? ""
                    : `<button type="button" class="secondary" data-archive-team="${escapeHtml(team.id)}">Archive Team</button>`
                }
              </div>
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
  teamRoot.querySelectorAll("[data-archive-team]").forEach((button) => {
    button.addEventListener("click", () => archiveTeam(button.dataset.archiveTeam));
  });

  projectRoot.innerHTML = state.selectedTeamId
    ? state.projects.length
      ? state.projects
          .map(
            (project) => `
              <div class="item">
                <strong>${escapeHtml(project.name)}</strong>
                <div class="muted">${escapeHtml(project.slug)} · ${escapeHtml(project.id)} · ${escapeHtml(project.archived_at ? "archived" : "active")}</div>
                ${
                  project.archived_at
                    ? ""
                    : `<button type="button" class="secondary" data-archive-project="${escapeHtml(project.id)}">Archive Project</button>`
                }
              </div>
            `,
          )
          .join("")
      : `<div class="muted">No projects for selected team</div>`
    : `<div class="muted">Select a team to manage projects</div>`;
  projectRoot.querySelectorAll("[data-archive-project]").forEach((button) => {
    button.addEventListener("click", () => archiveProject(button.dataset.archiveProject));
  });

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

  tenantInvitationRoot.innerHTML = state.selectedOrganizationId
    ? state.tenantInvitations.length
      ? state.tenantInvitations
          .map(
            (invitation) => `
              <div class="item">
                <strong>${escapeHtml(invitation.email)}</strong>
                <div class="muted">${escapeHtml(invitation.role)} · ${escapeHtml(invitation.status)} · expires ${escapeHtml(invitation.expires_at)}</div>
                <div class="muted">token ${escapeHtml(invitation.token)}</div>
                ${
                  invitation.status === "pending"
                    ? `<button type="button" class="secondary reject" data-revoke-invitation="${escapeHtml(invitation.id)}">Revoke Invitation</button>`
                    : ""
                }
              </div>
            `,
          )
          .join("")
      : `<div class="muted">No tenant invitations for selected organization</div>`
    : `<div class="muted">Select an organization to manage invitations</div>`;
  tenantInvitationRoot.querySelectorAll("[data-revoke-invitation]").forEach((button) => {
    button.addEventListener("click", () =>
      revokeTenantInvitation(button.dataset.revokeInvitation),
    );
  });
}

function renderTenantIsolationReadiness() {
  const report = state.tenantIsolationReadiness;
  if (!report) {
    tenantIsolationReadinessRoot.innerHTML = `<div class="muted">Tenant isolation readiness not loaded</div>`;
    return;
  }
  const counts = report.scoped_counts || {};
  const rls = report.rls || {};
  const productionRouting = report.production_routing || {};
  const attentionItems = report.attention_items || [];
  const tableCoverage = report.table_coverage || [];
  tenantIsolationReadinessRoot.innerHTML = `
    <div class="metric-grid compact-metrics">
      <div class="metric"><span>Status</span><strong>${escapeHtml(report.status || "unknown")}</strong></div>
      <div class="metric"><span>Score</span><strong>${formatInteger(report.readiness_score || 0)}</strong></div>
      <div class="metric"><span>Runtime Tenant</span><strong>${escapeHtml(report.runtime_tenant_mode || "unknown")}</strong></div>
      <div class="metric"><span>Prod Routing</span><strong>${escapeHtml(productionRouting.status || "unknown")}</strong></div>
      <div class="metric"><span>RLS</span><strong>${escapeHtml(rls.status || "unknown")}</strong></div>
    </div>
    <div class="item">
      <strong>TENANT BOUNDARY</strong>
      <div class="muted">Runtime tenant ${escapeHtml(report.runtime_tenant_id || "unknown")} · header fail-closed ${report.header_fail_closed ? "yes" : "no"} · membership scope ${report.membership_scope_enforced ? "enforced" : "missing"}</div>
      <div class="muted">Organizations ${formatInteger(counts.organizations || 0)} · Teams ${formatInteger(counts.teams || 0)} · Projects ${formatInteger(counts.projects || 0)} · Memberships ${formatInteger(counts.memberships || 0)} · Invitations ${formatInteger(counts.invitations || 0)}</div>
    </div>
    <div class="item">
      <strong>PRODUCTION ROUTING GATE</strong>
      <div class="muted">${escapeHtml(productionRouting.message || "tenant production routing gate is not reported")}</div>
      <div class="muted">cross-tenant routing ${productionRouting.cross_tenant_routing_supported ? "ready" : "missing"} · header fail-closed ${productionRouting.header_fail_closed ? "yes" : "no"} · membership scope ${productionRouting.membership_scope_enforced ? "yes" : "no"} · RLS ready ${productionRouting.rls_ready ? "yes" : "no"}</div>
    </div>
    <div class="item">
      <strong>TABLE COVERAGE</strong>
      <div class="muted">${formatInteger(tableCoverage.length)} tenant-scoped tables tracked · RLS required ${rls.required_for_production ? "yes" : "no"} · enabled ${rls.enabled ? "yes" : "no"} · forced ${rls.forced ? "yes" : "no"}</div>
      <div class="muted">Migration asset ${rls.migration_asset_present ? "present" : "missing"} · tenant context ${rls.tenant_context_configured ? "configured" : "missing"} · enabled tables ${formatInteger(rls.enabled_table_count || 0)}/${formatInteger(rls.tracked_table_count || tableCoverage.length)} · forced tables ${formatInteger(rls.forced_table_count || 0)}/${formatInteger(rls.tracked_table_count || tableCoverage.length)}</div>
      <div class="muted">${escapeHtml(
        tableCoverage
          .slice(0, 10)
          .map((table) => `${table.table}:${table.rls_forced ? "forced-rls" : table.rls_enabled ? "rls" : table.store_filters_tenant ? "tenant-filtered" : "missing"}`)
          .join(", ") || "none",
      )}</div>
    </div>
    <div class="item">
      <strong>ISOLATION ATTENTION</strong>
      ${
        attentionItems.length
          ? attentionItems.map((item) => `<div class="muted">${escapeHtml(item.severity)} · ${escapeHtml(item.kind)} · ${escapeHtml(item.message)}</div>`).join("")
          : `<div class="muted">No tenant isolation attention items</div>`
      }
    </div>
  `;
}

function renderObservability() {
  const observability = state.observability;
  if (!observability) {
    observabilityRoot.innerHTML = `<div class="muted">Observability data is not loaded.</div>`;
    return;
  }
  const backpressure = observability.backpressure || {};
  const telemetry = observability.telemetry || {};
  const errorEvents = observability.recent_error_events || [];
  const collectorReadiness = state.observabilityCollectorReadiness;
  const collectorAttention = collectorReadiness?.attention_items || [];
  const collectorSignalPaths = collectorReadiness?.signal_paths || [];
  const collectorProductionOps = collectorReadiness?.production_ops || {};
  const collectorDeploymentReadiness = collectorReadiness?.deployment_readiness || {};
  const remediationSupervision = collectorReadiness?.remediation_supervision || {};
  const remediationPlan = state.observabilityRemediationPlan;
  const remediationPlanActions = remediationPlan?.actions || [];
  const remediation = state.observabilityRemediation;
  const schedulerSummary = state.schedulerSummary;
  const schedulerAttention = schedulerSummary?.attention_items || [];
  const schedulerRuns = schedulerSummary?.recent_runs || [];
  const schedulerDeployment = schedulerSummary?.deployment_readiness || {};
  const schedulerDuePlan = state.schedulerDuePlan;
  const schedulerDuePlanActions = schedulerDuePlan?.actions || [];
  const schedulerDueRun = state.schedulerDueRun;
  observabilityRoot.innerHTML = `
    <div class="metric-grid">
      <div class="metric">
        <span>Backpressure</span>
        <strong>${escapeHtml(backpressure.status || "unknown")}</strong>
      </div>
      <div class="metric">
        <span>Pending approvals</span>
        <strong>${formatInteger(backpressure.pending_approvals || 0)}</strong>
      </div>
      <div class="metric">
        <span>Queued jobs</span>
        <strong>${formatInteger(backpressure.queued_jobs || 0)}</strong>
      </div>
      <div class="metric">
        <span>Failed signals</span>
        <strong>${formatInteger(errorEvents.length)}</strong>
      </div>
    </div>
    <dl>
      <dt>Telemetry</dt>
      <dd>${escapeHtml(telemetry.service_name || "unknown")} · ${escapeHtml(telemetry.otlp_enabled ? "OTLP enabled" : "OTLP disabled")} · sample ${escapeHtml(String(telemetry.sample_ratio ?? "unknown"))}</dd>
      <dt>Sessions</dt>
      <dd>${escapeHtml(formatCounts(observability.sessions_by_status))}</dd>
      <dt>Tool calls</dt>
      <dd>${escapeHtml(formatCounts(observability.tool_calls_by_status))}</dd>
      <dt>Approvals</dt>
      <dd>${escapeHtml(formatCounts(observability.approvals_by_status))}</dd>
      <dt>Execution jobs</dt>
      <dd>${escapeHtml(formatCounts(observability.execution_jobs_by_status))}</dd>
      <dt>Event categories</dt>
      <dd>${escapeHtml(formatCounts(observability.event_categories))}</dd>
      <dt>Oldest queued job</dt>
      <dd>${escapeHtml(backpressure.oldest_queued_job_age_seconds == null ? "none" : `${backpressure.oldest_queued_job_age_seconds}s`)}</dd>
    </dl>
    ${
      collectorReadiness
        ? `<h4>Collector Readiness</h4>
          <div class="metric-grid compact-metrics">
            <div class="metric"><span>Status</span><strong>${escapeHtml(collectorReadiness.status)}</strong></div>
            <div class="metric"><span>OTLP</span><strong>${escapeHtml(collectorReadiness.otlp_enabled ? "enabled" : "disabled")}</strong></div>
            <div class="metric"><span>Endpoint</span><strong>${escapeHtml(collectorReadiness.endpoint_configured ? "configured" : "missing")}</strong></div>
            <div class="metric"><span>Health</span><strong>${escapeHtml(collectorReadiness.health_check?.status || "unknown")}</strong></div>
          </div>
          <dl>
            <dt>Collector endpoint</dt>
            <dd>${escapeHtml(collectorReadiness.endpoint || "none")}</dd>
            <dt>Health message</dt>
            <dd>${escapeHtml(collectorReadiness.health_check?.message || "none")}</dd>
            <dt>Production ops</dt>
            <dd>${escapeHtml(collectorProductionOps.status || "unknown")} · blocked ${collectorProductionOps.production_blocked ? "yes" : "no"} · paths ${formatInteger(collectorProductionOps.configured_signal_path_count || 0)}/${formatInteger(collectorProductionOps.signal_path_count || 0)} · health ${escapeHtml(collectorProductionOps.health_status || "unknown")}</dd>
            <dt>Production message</dt>
            <dd>${escapeHtml(collectorProductionOps.message || "collector production ops are not reported")}</dd>
            <dt>Deployment validation</dt>
            <dd>${escapeHtml(collectorDeploymentReadiness.status || "unknown")} · blocked ${collectorDeploymentReadiness.production_blocked ? "yes" : "no"} · validated ${collectorDeploymentReadiness.deployment_validated ? "yes" : "no"} · healthy ${collectorDeploymentReadiness.latest_validation_healthy ? "yes" : "no"} · controller ${collectorDeploymentReadiness.controller_configured ? "configured" : "missing"} · latest ${escapeHtml(collectorDeploymentReadiness.latest_validation_at || "none")}</dd>
            <dt>Deployment controller</dt>
            <dd>required ${collectorDeploymentReadiness.controller_required ? "yes" : "no"} · status ${escapeHtml(collectorDeploymentReadiness.latest_controller_status || "none")} · validated ${collectorDeploymentReadiness.latest_controller_validated ? "yes" : "no"}</dd>
            <dt>Deployment message</dt>
            <dd>${escapeHtml(collectorDeploymentReadiness.message || "collector deployment validation is not reported")}</dd>
            <dt>Remediation supervision</dt>
            <dd>${escapeHtml(remediationSupervision.status || "unknown")} · blocked ${remediationSupervision.production_blocked ? "yes" : "no"} · required ${remediationSupervision.required ? "yes" : "no"} · controller ${remediationSupervision.controller_configured ? "configured" : "missing"} · latest ${escapeHtml(remediationSupervision.latest_controller_run_at || "none")}</dd>
            <dt>Remediation message</dt>
            <dd>${escapeHtml(remediationSupervision.message || "observability remediation supervision is not reported")}</dd>
          </dl>
          ${
            collectorSignalPaths.length
              ? `<table class="usage-table">
                  <thead>
                    <tr>
                      <th>Signal</th>
                      <th>Status</th>
                      <th>URL</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${collectorSignalPaths
                      .map(
                        (path) => `
                          <tr>
                            <td>${escapeHtml(path.signal)}</td>
                            <td>${escapeHtml(path.status)}</td>
                            <td>${escapeHtml(path.url || "none")}</td>
                          </tr>
                        `,
                      )
                      .join("")}
                  </tbody>
                </table>`
              : `<div class="muted">No collector signal paths.</div>`
          }
          ${
            collectorAttention.length
              ? `<table class="usage-table">
                  <thead>
                    <tr>
                      <th>Severity</th>
                      <th>Signal</th>
                      <th>Message</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${collectorAttention
                      .map(
                        (item) => `
                          <tr>
                            <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                            <td>${escapeHtml(item.kind)}</td>
                            <td>${escapeHtml(item.message)}</td>
                          </tr>
                        `,
                      )
                      .join("")}
                  </tbody>
                </table>`
              : `<div class="muted">No collector readiness attention items.</div>`
          }
          <div class="muted">Generated: ${escapeHtml(collectorReadiness.generated_at)}</div>`
        : ""
    }
    ${
      remediationPlan
        ? `<h4>Remediation Plan</h4>
          <div class="metric-grid compact-metrics">
            <div class="metric"><span>Status</span><strong>${escapeHtml(remediationPlan.status)}</strong></div>
            <div class="metric"><span>Auto</span><strong>${formatInteger(remediationPlan.auto_action_count)}</strong></div>
            <div class="metric"><span>Manual</span><strong>${formatInteger(remediationPlan.manual_action_count)}</strong></div>
            <div class="metric"><span>Config</span><strong>${formatInteger(remediationPlan.configuration_action_count)}</strong></div>
          </div>
          ${
            remediationPlanActions.length
              ? `<table class="usage-table">
                  <thead>
                    <tr>
                      <th>Severity</th>
                      <th>Mode</th>
                      <th>Action</th>
                      <th>Reason</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${remediationPlanActions
                      .map(
                        (action) => `
                          <tr>
                            <td><span class="budget-status ${escapeHtml(action.severity)}">${escapeHtml(action.severity)}</span></td>
                            <td>${escapeHtml(action.mode)}</td>
                            <td>${escapeHtml(action.action)}</td>
                            <td>${escapeHtml(action.reason)}</td>
                          </tr>
                        `,
                      )
                      .join("")}
                  </tbody>
                </table>`
              : `<div class="muted">No remediation actions planned.</div>`
          }
          <div class="muted">Generated: ${escapeHtml(remediationPlan.generated_at)}</div>`
        : ""
    }
    ${
      schedulerSummary
        ? `<h4>Scheduler Orchestration</h4>
          <div class="metric-grid compact-metrics">
            <div class="metric"><span>Status</span><strong>${escapeHtml(schedulerSummary.status)}</strong></div>
            <div class="metric"><span>Recent Runs</span><strong>${formatInteger(schedulerSummary.recent_run_count)}</strong></div>
            <div class="metric"><span>Last Actions</span><strong>${formatInteger(schedulerSummary.last_run_action_count)}</strong></div>
            <div class="metric"><span>Attention</span><strong>${formatInteger(schedulerAttention.length)}</strong></div>
            <div class="metric"><span>Deploy</span><strong>${escapeHtml(schedulerDeployment.status || "unknown")}</strong></div>
          </div>
          <dl>
            <dt>Last run</dt>
            <dd>${escapeHtml(schedulerSummary.last_run_at || "none")} · ${escapeHtml(schedulerSummary.last_run_status || "none")}</dd>
            <dt>Deployment readiness</dt>
            <dd>${escapeHtml(schedulerDeployment.message || "scheduler deployment readiness is not reported")}</dd>
            <dt>Scheduler auth</dt>
            <dd>subject secret ${schedulerDeployment.subject_from_secret ? "yes" : "no"} · roles secret ${schedulerDeployment.roles_from_secret ? "yes" : "no"} · token secret ${schedulerDeployment.token_from_secret ? "yes" : "no"} · token runtime ${schedulerDeployment.shared_token_runtime_configured ? "yes" : "no"} · hardcoded demo headers ${schedulerDeployment.hardcoded_admin_headers_absent ? "absent" : "present"}</dd>
          </dl>
          ${
            schedulerAttention.length
              ? `<table class="usage-table">
                  <thead>
                    <tr>
                      <th>Severity</th>
                      <th>Kind</th>
                      <th>Message</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${schedulerAttention
                      .map(
                        (item) => `
                          <tr>
                            <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                            <td>${escapeHtml(item.kind)}</td>
                            <td>${escapeHtml(item.message)}</td>
                          </tr>
                        `,
                      )
                      .join("")}
                  </tbody>
                </table>`
              : `<div class="muted">No scheduler attention items.</div>`
          }
          ${
            schedulerRuns.length
              ? `<table class="usage-table">
                  <thead>
                    <tr>
                      <th>Run</th>
                      <th>Status</th>
                      <th>Teams</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${schedulerRuns
                      .map(
                        (run) => `
                          <tr>
                            <td>${escapeHtml(run.created_at)}</td>
                            <td>${escapeHtml(run.status)}</td>
                            <td>${formatInteger(run.team_count)}</td>
                            <td>${escapeHtml((run.actions || []).join(", ") || "no action")}</td>
                          </tr>
                        `,
                      )
                      .join("")}
                  </tbody>
                </table>`
              : `<div class="muted">No scheduler run history yet.</div>`
          }
          <div class="muted">Generated: ${escapeHtml(schedulerSummary.generated_at)}</div>`
        : ""
    }
    ${
      schedulerDuePlan
        ? `<h4>Scheduler Due Plan</h4>
          <div class="metric-grid compact-metrics">
            <div class="metric"><span>Status</span><strong>${escapeHtml(schedulerDuePlan.status)}</strong></div>
            <div class="metric"><span>Teams</span><strong>${formatInteger(schedulerDuePlan.team_count)}</strong></div>
            <div class="metric"><span>Items</span><strong>${formatInteger(schedulerDuePlan.item_count)}</strong></div>
            <div class="metric"><span>Actionable</span><strong>${formatInteger(schedulerDuePlan.actionable_count)}</strong></div>
          </div>
          ${
            schedulerDuePlanActions.length
              ? `<table class="usage-table">
                  <thead>
                    <tr>
                      <th>Area</th>
                      <th>Action</th>
                      <th>Status</th>
                      <th>Due</th>
                      <th>Targets</th>
                      <th>Reason</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${schedulerDuePlanActions
                      .map(
                        (action) => `
                          <tr>
                            <td>${escapeHtml(action.area)}</td>
                            <td>${escapeHtml(action.action)}</td>
                            <td><span class="budget-status ${escapeHtml(action.severity)}">${escapeHtml(action.status)}</span></td>
                            <td>${formatInteger(action.due_count)}</td>
                            <td>${formatInteger(action.target_count)}</td>
                            <td>${escapeHtml(action.reason)}</td>
                          </tr>
                        `,
                      )
                      .join("")}
                  </tbody>
                </table>`
              : `<div class="muted">No scheduler actions planned.</div>`
          }
          <div class="muted">Generated: ${escapeHtml(schedulerDuePlan.generated_at)}</div>`
        : ""
    }
    <h4>Recent Error Events</h4>
    ${
      errorEvents.length
        ? errorEvents
            .map(
              (event) => `
                <div class="item">
                  <strong>${escapeHtml(event.event_type)}</strong>
                  <div class="muted">${escapeHtml(event.status)} · seq ${formatInteger(event.seq)} · ${escapeHtml(event.created_at)}</div>
                  <div class="muted">${escapeHtml(event.session_id)}</div>
                </div>
              `,
            )
            .join("")
        : `<div class="muted">No recent error events</div>`
    }
      ${
        state.observabilityCollectorValidation
          ? `<div class="item">
              <strong>Collector Deployment Validation</strong>
              <pre>${escapeHtml(JSON.stringify(state.observabilityCollectorValidation, null, 2))}</pre>
            </div>`
          : ""
      }
      ${
        remediation
        ? `<h4>Remediation Run</h4>
          <div class="item">
            <strong>${escapeHtml(remediation.status)}</strong>
            <div class="muted">${escapeHtml(remediation.ran_at)} · ${escapeHtml((remediation.actions || []).join(", ") || "no action")}</div>
            <pre>${escapeHtml(
              JSON.stringify(
                {
                  before: remediation.before,
                  after: remediation.after,
                  controller_configured: remediation.controller_configured,
                  controller_execution: remediation.controller_execution,
                  approval_escalation_run: remediation.approval_escalation_run,
                  codex_app_server_stale_polls: remediation.codex_app_server_stale_polls,
                },
                null,
                2,
              ),
            )}</pre>
          </div>`
        : ""
    }
    ${
      schedulerDueRun
        ? `<h4>Scheduler Due Run</h4>
          <div class="item">
            <strong>${escapeHtml(schedulerDueRun.status)}</strong>
            <div class="muted">${escapeHtml(schedulerDueRun.checked_at)} · ${formatInteger(schedulerDueRun.team_count)} teams · ${escapeHtml((schedulerDueRun.actions || []).join(", ") || "no action")}</div>
            <pre>${escapeHtml(
              JSON.stringify(
                {
                  provider_policy_gate: schedulerDueRun.provider_policy_gate,
                  policy_rollout: schedulerDueRun.policy_rollout,
                  approval_escalations: schedulerDueRun.approval_escalations,
                  agent_releases: schedulerDueRun.agent_releases,
                  mcp_health_runs: schedulerDueRun.mcp_health_runs,
                  mcp_rollout_runs: schedulerDueRun.mcp_rollout_runs,
                  codex_app_server_stale_polls: schedulerDueRun.codex_app_server_stale_polls,
                  cost_alert_delivery: schedulerDueRun.cost_alert_delivery,
                  usage_finance_export: schedulerDueRun.usage_finance_export,
                  remote_computer_reclaim: schedulerDueRun.remote_computer_reclaim,
                  remote_computer_sidecar_supervision:
                    schedulerDueRun.remote_computer_sidecar_supervision,
                },
                null,
                2,
              ),
            )}</pre>
          </div>`
        : ""
    }
  `;
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
  const trend = state.usageTrend;
  const budgetPressure = trend?.budget_pressure || {};
  const forecast = trend?.forecast || {};
  const forecastHorizons = forecast.horizons || [];
  const budgetForecasts = forecast.provider_budget_exhaustion || [];
  const usageExportStatus = state.usageExportStatus;
  const usageExportDelivery = state.usageExportDelivery;
  const financeSummary = state.usageFinanceSummary;
  const financeOperations = state.usageFinanceOperations;
  const financeOperationsRun = state.usageFinanceOperationsRun;
  const financeAttention = financeSummary?.attention_items || [];
  const financeOperationsAttention = financeOperations?.attention_items || [];
  const financeProductionClose = financeOperations?.production_close || {};
  const toolEntries = Object.entries(usage.by_tool || {}).sort(
    ([, left], [, right]) => Number(right.call_count || 0) - Number(left.call_count || 0),
  );
  const averageToolDurationMs =
    usage.tool_call_count > 0 ? usage.total_tool_duration_ms / usage.tool_call_count : 0;
  usageRoot.innerHTML = `
    ${
      financeOperations
        ? `<div class="detail-panel">
            <h4>Finance Operations</h4>
            <div class="metric-grid compact-metrics">
              <div class="metric"><span>Operations Status</span><strong>${escapeHtml(financeOperations.status || "unknown")}</strong></div>
              <div class="metric"><span>Readiness</span><strong>${formatInteger(financeOperations.readiness_score)}%</strong></div>
              <div class="metric"><span>Open Alerts</span><strong>${formatInteger(financeOperations.open_alert_count)}</strong></div>
              <div class="metric"><span>Unacknowledged</span><strong>${formatInteger(financeOperations.unacknowledged_alert_count)}</strong></div>
              <div class="metric"><span>Rollups</span><strong>${escapeHtml(financeOperations.rollup_status || "unknown")}</strong></div>
              <div class="metric"><span>Export</span><strong>${escapeHtml(financeOperations.export_status || "unknown")}</strong></div>
              <div class="metric"><span>Alert Delivery</span><strong>${escapeHtml(financeOperations.alert_delivery_status || "unknown")}</strong></div>
              <div class="metric"><span>Prod Close</span><strong>${escapeHtml(financeProductionClose.status || "unknown")}</strong></div>
              <div class="metric"><span>Routes</span><strong>${formatInteger(financeOperations.active_alert_route_count)}</strong></div>
            </div>
            <dl>
              <dt>Production close gate</dt>
              <dd>${escapeHtml(financeProductionClose.message || "finance production close gate is not reported")}</dd>
              <dt>Production close evidence</dt>
              <dd>rollup ${financeProductionClose.rollup_fresh ? "fresh" : "not fresh"} · export target ${financeProductionClose.export_target_configured ? "ready" : "missing"} · export recent ${financeProductionClose.export_recent ? "yes" : "no"} · alerts delivered ${financeProductionClose.alert_delivery_ready ? "yes" : "no"} · critical ack ${financeProductionClose.critical_alerts_acknowledged ? "yes" : "no"} · controller ${financeProductionClose.close_controller_configured ? "configured" : "missing"}</dd>
              <dt>Close controller</dt>
              <dd>required ${financeProductionClose.close_controller_required ? "yes" : "no"} · status ${escapeHtml(financeProductionClose.latest_close_controller_status || "none")} · closed ${financeProductionClose.latest_close_controller_closed ? "yes" : "no"}</dd>
              <dt>Last finance export</dt>
              <dd>${renderFinanceOperationAudit(financeOperations.last_finance_export)}</dd>
              <dt>Last alert delivery</dt>
              <dd>${renderFinanceOperationAudit(financeOperations.last_alert_delivery)}</dd>
              <dt>Last alert acknowledgement</dt>
              <dd>${renderFinanceOperationAudit(financeOperations.last_alert_acknowledgement)}</dd>
              <dt>Runbook actions</dt>
              <dd>${escapeHtml((financeOperations.runbook_actions || []).join(" · ") || "No finance operations action required")}</dd>
            </dl>
            ${
              financeOperationsAttention.length
                ? `<table class="usage-table">
                    <thead>
                      <tr>
                        <th>Severity</th>
                        <th>Signal</th>
                        <th>Provider</th>
                        <th>Message</th>
                      </tr>
                    </thead>
                    <tbody>
                      ${financeOperationsAttention
                        .map(
                          (item) => `
                            <tr>
                              <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                              <td>${escapeHtml(item.kind)}</td>
                              <td>${escapeHtml(item.provider_name || "global")}</td>
                              <td>${escapeHtml(item.message)}</td>
                            </tr>
                          `,
                        )
                        .join("")}
                    </tbody>
                  </table>`
                : `<div class="muted">No finance operations attention items.</div>`
            }
            <div class="muted">Generated: ${escapeHtml(financeOperations.generated_at)}</div>
          </div>`
        : ""
    }
    ${
      financeOperationsRun
        ? `<div class="item">
            <strong>Finance operations run: ${escapeHtml(financeOperationsRun.status)}</strong>
            <div class="muted">${escapeHtml(financeOperationsRun.ran_at)} · ${(financeOperationsRun.actions || []).map(escapeHtml).join(" · ") || "no action"}</div>
            <div class="muted">Before ${escapeHtml(financeOperationsRun.before?.status || "unknown")} → after ${escapeHtml(financeOperationsRun.after?.status || "unknown")}</div>
            <div class="muted">Rollup ${escapeHtml(financeOperationsRun.rollup_created ? "created" : "not created")} · alerts ${escapeHtml(financeOperationsRun.cost_alert_delivery?.status || "not run")} · export ${escapeHtml(financeOperationsRun.finance_export_delivery?.status || "not run")}</div>
            <div class="muted">Close controller ${escapeHtml(financeOperationsRun.close_controller_configured ? "configured" : "not configured")} · ${escapeHtml(financeOperationsRun.close_controller_execution?.status || "skipped")}</div>
          </div>`
        : ""
    }
    ${
      financeSummary
        ? `<div class="detail-panel">
            <div class="metric-grid compact-metrics">
              <div class="metric"><span>Finance Status</span><strong>${escapeHtml(financeSummary.budget_pressure_status || "ok")}</strong></div>
              <div class="metric"><span>Alerts</span><strong>${formatInteger(financeSummary.alert_count)}</strong></div>
              <div class="metric"><span>Routes</span><strong>${formatInteger(financeSummary.active_alert_route_count)}/${formatInteger(financeSummary.alert_route_count)}</strong></div>
              <div class="metric"><span>Rollups</span><strong>${formatInteger(financeSummary.rollup_count)}</strong></div>
              <div class="metric"><span>7d Forecast</span><strong>${formatOptionalCents(financeSummary.forecast_7d_cost_cents)}</strong></div>
              <div class="metric"><span>30d Forecast</span><strong>${formatOptionalCents(financeSummary.forecast_30d_cost_cents)}</strong></div>
              <div class="metric"><span>Export Target</span><strong>${escapeHtml(financeSummary.finance_export_target_configured ? "ready" : "missing")}</strong></div>
              <div class="metric"><span>Attention</span><strong>${formatInteger(financeAttention.length)}</strong></div>
            </div>
            <dl>
              <dt>Top provider</dt>
              <dd>${
                financeSummary.top_provider_by_cost
                  ? `${escapeHtml(financeSummary.top_provider_by_cost.provider_name)} · ${formatCents(financeSummary.top_provider_by_cost.estimated_cost_cents)} · ${formatInteger(financeSummary.top_provider_by_cost.total_tokens)} tokens`
                  : "none"
              }</dd>
              <dt>Latest rollup</dt>
              <dd>${escapeHtml(financeSummary.latest_rollup_at || "none")} · ${escapeHtml(financeSummary.latest_rollup_age_hours == null ? "no age" : `${financeSummary.latest_rollup_age_hours}h old`)}</dd>
              <dt>Recommendations</dt>
              <dd>${escapeHtml((financeSummary.recommendations || []).join(" · ") || "No active finance recommendation")}</dd>
            </dl>
            ${
              financeAttention.length
                ? `<table class="usage-table">
                    <thead>
                      <tr>
                        <th>Severity</th>
                        <th>Signal</th>
                        <th>Provider</th>
                        <th>Message</th>
                      </tr>
                    </thead>
                    <tbody>
                      ${financeAttention
                        .map(
                          (item) => `
                            <tr>
                              <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                              <td>${escapeHtml(item.kind)}</td>
                              <td>${escapeHtml(item.provider_name || "global")}</td>
                              <td>${escapeHtml(item.message)}</td>
                            </tr>
                          `,
                        )
                        .join("")}
                    </tbody>
                  </table>`
                : `<div class="muted">No finance attention items.</div>`
            }
            <div class="muted">Generated: ${escapeHtml(financeSummary.generated_at)}</div>
          </div>`
        : ""
    }
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
    <h4>Cost Trend</h4>
    ${
      trend
        ? `<div class="metric-grid">
            <div class="metric">
              <span>Comparison</span>
              <strong>${escapeHtml(trend.comparison_basis)}</strong>
            </div>
            <div class="metric">
              <span>Cost delta</span>
              <strong>${escapeHtml(formatSignedCents(trend.cost_delta_cents))}</strong>
            </div>
            <div class="metric">
              <span>Token delta</span>
              <strong>${escapeHtml(formatSignedInteger(trend.token_delta))}</strong>
            </div>
            <div class="metric">
              <span>Budget pressure</span>
              <strong>${escapeHtml(budgetPressure.highest_status || "ok")}</strong>
            </div>
          </div>
          <table class="usage-table">
            <thead>
              <tr>
                <th>Window</th>
                <th>Cost</th>
                <th>Tokens</th>
                <th>Tool calls</th>
              </tr>
            </thead>
            <tbody>
              ${renderTrendPeriodRow("Latest", trend.latest_period)}
              ${renderTrendPeriodRow("Previous", trend.previous_period)}
            </tbody>
          </table>
          <dl>
            <dt>Delta rates</dt>
            <dd>${escapeHtml(formatSignedPercent(trend.cost_delta_percent))} cost · ${escapeHtml(formatSignedPercent(trend.token_delta_percent))} tokens · ${escapeHtml(formatSignedPercent(trend.tool_call_delta_percent))} tool calls</dd>
            <dt>Top provider</dt>
            <dd>${
              trend.top_provider_by_cost
                ? `${escapeHtml(trend.top_provider_by_cost.provider_name)} · ${escapeHtml(formatCents(trend.top_provider_by_cost.estimated_cost_cents))} · ${escapeHtml(formatInteger(trend.top_provider_by_cost.total_tokens))} tokens`
                : "none"
            }</dd>
            <dt>Budget pressure</dt>
            <dd>${formatInteger(budgetPressure.pressure_count)} pressured of ${formatInteger(budgetPressure.total_budgeted_providers)} budgeted providers · ${formatInteger(budgetPressure.warning_count)} warning · ${formatInteger(budgetPressure.critical_count)} critical · peak ${escapeHtml(formatOptionalPercent(budgetPressure.highest_used_percent))}</dd>
            <dt>Recommendations</dt>
            <dd>${escapeHtml((trend.recommendations || []).join(" · ") || "No active recommendation")}</dd>
          </dl>`
        : `<div class="muted">Usage trend data is not loaded.</div>`
    }
    <h4>Cost Forecast</h4>
    ${
      forecastHorizons.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Horizon</th>
                <th>Projected cost</th>
                <th>Projected tokens</th>
                <th>Projected tool calls</th>
              </tr>
            </thead>
            <tbody>
              ${forecastHorizons
                .map(
                  (horizon) => `
                    <tr>
                      <td>${formatInteger(horizon.days)}d</td>
                      <td>${formatCents(horizon.projected_cost_cents)}</td>
                      <td>${formatInteger(horizon.projected_tokens)}</td>
                      <td>${formatInteger(horizon.projected_tool_calls)}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>
          <div class="muted">Basis: ${escapeHtml(forecast.basis || "unknown")}</div>`
        : `<div class="muted">No forecast horizon data yet.</div>`
    }
    ${
      budgetForecasts.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Provider</th>
                <th>Status</th>
                <th>Daily run rate</th>
                <th>Days to limit</th>
              </tr>
            </thead>
            <tbody>
              ${budgetForecasts
                .map(
                  (forecastRow) => `
                    <tr>
                      <td>${escapeHtml(forecastRow.provider_name)}</td>
                      <td><span class="budget-status ${escapeHtml(forecastRow.status)}">${escapeHtml(forecastRow.status)}</span></td>
                      <td>${formatCents(forecastRow.current_daily_cost_cents)} / ${formatCents(forecastRow.daily_cost_limit_cents)}</td>
                      <td>${escapeHtml(forecastRow.projected_days_to_limit == null ? "unknown" : `${Number(forecastRow.projected_days_to_limit).toFixed(1)}d`)} · ${escapeHtml(forecastRow.projected_exhaustion_at || "no projection")}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No cost-limit forecasts configured.</div>`
    }
    ${
      usageExportStatus
        ? `<div class="item">
            <strong>Finance CSV export: ${escapeHtml(usageExportStatus.status)}</strong>
            <div class="muted">${formatInteger(usageExportStatus.bytes)} bytes generated from /api/usage/export.csv</div>
            <pre>${escapeHtml(usageExportStatus.preview)}</pre>
          </div>`
        : ""
    }
    ${
      usageExportDelivery
        ? `<div class="item">
            <strong>Finance CSV delivery: ${escapeHtml(usageExportDelivery.status)}</strong>
            <div class="muted">${escapeHtml(usageExportDelivery.channel)} · ${escapeHtml(usageExportDelivery.target_configured ? "target configured" : "target not configured")} · ${escapeHtml(usageExportDelivery.scheduled ? "scheduled" : "manual")} · ${formatInteger(usageExportDelivery.bytes)} bytes</div>
            <div class="muted">${formatInteger(usageExportDelivery.provider_count)} providers · ${formatInteger(usageExportDelivery.budget_pressure_count)} budget pressure · ${formatInteger(usageExportDelivery.rollup_count)} rollups</div>
          </div>`
        : ""
    }
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

function formatCounts(counts) {
  const entries = Object.entries(counts || {}).sort((left, right) => {
    const countDelta = Number(right[1] || 0) - Number(left[1] || 0);
    return countDelta || left[0].localeCompare(right[0]);
  });
  return entries.length
    ? entries.map(([key, value]) => `${key}: ${formatInteger(value)}`).join(" · ")
    : "none";
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

function formatSignedCents(value) {
  if (value === null || value === undefined) {
    return "n/a";
  }
  const number = Number(value || 0);
  return `${number >= 0 ? "+" : ""}${number.toFixed(2)} cents`;
}

function formatSignedInteger(value) {
  if (value === null || value === undefined) {
    return "n/a";
  }
  const number = Number(value || 0);
  return `${number >= 0 ? "+" : ""}${formatInteger(number)}`;
}

function formatSignedPercent(value) {
  if (value === null || value === undefined) {
    return "n/a";
  }
  const number = Number(value || 0);
  return `${number >= 0 ? "+" : ""}${number.toFixed(1)}%`;
}

function renderTrendPeriodRow(label, period) {
  if (!period) {
    return `
      <tr>
        <td>${escapeHtml(label)}</td>
        <td colspan="3"><span class="muted">No comparison window</span></td>
      </tr>
    `;
  }
  return `
    <tr>
      <td>${escapeHtml(label)}<br /><span class="muted">${escapeHtml(period.period_start)} to ${escapeHtml(period.period_end)}</span></td>
      <td>${formatCents(period.cost_cents)}</td>
      <td>${formatInteger(period.total_tokens)}</td>
      <td>${formatInteger(period.tool_calls)}</td>
    </tr>
  `;
}

function renderFinanceOperationAudit(audit) {
  if (!audit) {
    return "none";
  }
  const subject = audit.subject ? ` · ${escapeHtml(audit.subject)}` : "";
  return `${escapeHtml(audit.status || "recorded")} · ${escapeHtml(audit.created_at)}${subject}`;
}

function renderProviderPolicyGateRun(run) {
  if (!run) {
    return "none";
  }
  return `${escapeHtml(run.status)} · ${escapeHtml(run.ran_at)} · ${formatInteger(run.failed_count)} failed · ${formatInteger(run.warning_count)} warning`;
}

function formatDurationMs(value) {
  const milliseconds = Number(value || 0);
  if (milliseconds >= 1000) {
    return `${(milliseconds / 1000).toFixed(2)}s`;
  }
  return `${milliseconds.toFixed(0)}ms`;
}

function formatDurationSeconds(value) {
  const seconds = Number(value || 0);
  if (seconds >= 3600) {
    return `${(seconds / 3600).toFixed(1)}h`;
  }
  if (seconds >= 60) {
    return `${(seconds / 60).toFixed(1)}m`;
  }
  return `${seconds.toFixed(0)}s`;
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
      <dt>Active revision</dt>
      <dd>${escapeHtml(runtime.active_revision_id || "config baseline")}</dd>
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
  if (state.policyScheduledRolloutRun) {
    policyDecisionRoot.innerHTML += `
      <div class="item">
        <strong>Scheduled rollout run: ${escapeHtml(state.policyScheduledRolloutRun.status)}</strong>
        <div class="muted">${escapeHtml(state.policyScheduledRolloutRun.reason || "")}</div>
        <pre>${escapeHtml(JSON.stringify(state.policyScheduledRolloutRun, null, 2))}</pre>
      </div>
    `;
  }
  if (state.policyRollback) {
    policyDecisionRoot.innerHTML += `
      <div class="item">
        <strong>Policy rollback complete</strong>
        <div class="muted">${escapeHtml(state.policyRollback.rolled_back_from_revision_id)} -> ${escapeHtml(state.policyRollback.active_revision_id)}</div>
        <pre>${escapeHtml(JSON.stringify(state.policyRollback, null, 2))}</pre>
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
  rollbackPolicyRolloutButton.disabled =
    Boolean(state.policyRuntime?.rollout_active) || !state.policyRuntime?.active_revision_id;
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
  const summary = state.providerSummary;
  const attentionItems = summary?.attention_items || [];
  const policyGate = state.providerPolicyGate;
  const policyGateRuns = state.providerPolicyGateRuns;
  const providerDeploymentValidation = state.providerDeploymentValidation;
  const deploymentReadiness = summary?.deployment_readiness || {};
  const productionRolloutRun = state.providerProductionRolloutRun;
  const policyGateChecks = policyGate?.checks || [];
  const policyGateRunAttention = policyGateRuns?.attention_items || [];
  const policyGateHtml = policyGate
    ? `
      <div class="detail-panel">
        <h4>Provider Policy Gate</h4>
        <div class="metric-grid compact-metrics">
          <div class="metric"><span>Status</span><strong>${escapeHtml(policyGate.status)}</strong></div>
          <div class="metric"><span>Providers</span><strong>${formatInteger(policyGate.provider_count)}</strong></div>
          <div class="metric"><span>Passed</span><strong>${formatInteger(policyGate.passed_count)}</strong></div>
          <div class="metric"><span>Failed</span><strong>${formatInteger(policyGate.failed_count)}</strong></div>
          <div class="metric"><span>Warnings</span><strong>${formatInteger(policyGate.warning_count)}</strong></div>
        </div>
        ${
          policyGateChecks.length
            ? `<table class="compact-table">
                <thead>
                  <tr>
                    <th>Provider</th>
                    <th>Gate</th>
                    <th>Blockers</th>
                    <th>Warnings</th>
                    <th>Recommendations</th>
                  </tr>
                </thead>
                <tbody>
                  ${policyGateChecks
                    .map(
                      (check) => `
                        <tr>
                          <td>${escapeHtml(check.provider_name)}</td>
                          <td><span class="budget-status ${escapeHtml(check.gate_status)}">${escapeHtml(check.gate_status)}</span></td>
                          <td>${escapeHtml((check.blockers || []).join("; ") || "none")}</td>
                          <td>${escapeHtml((check.warnings || []).join("; ") || "none")}</td>
                          <td>${escapeHtml((check.recommendations || []).join("; ") || "none")}</td>
                        </tr>
                      `,
                    )
                    .join("")}
                </tbody>
              </table>`
            : `<div class="muted">No provider policy gate checks.</div>`
        }
        <div class="muted">Generated: ${escapeHtml(policyGate.generated_at)}</div>
      </div>
    `
    : "";
  const policyGateRunsHtml = policyGateRuns
    ? `
      <div class="detail-panel">
        <h4>Provider Gate Runs</h4>
        <div class="metric-grid compact-metrics">
          <div class="metric"><span>Runs</span><strong>${formatInteger(policyGateRuns.run_count)}</strong></div>
          <div class="metric"><span>Passed</span><strong>${formatInteger(policyGateRuns.passed_run_count)}</strong></div>
          <div class="metric"><span>Failed</span><strong>${formatInteger(policyGateRuns.failed_run_count)}</strong></div>
          <div class="metric"><span>Warnings</span><strong>${formatInteger(policyGateRuns.warning_run_count)}</strong></div>
        </div>
        <dl>
          <dt>Latest run</dt>
          <dd>${renderProviderPolicyGateRun(policyGateRuns.latest_run)}</dd>
          <dt>Production enforcement</dt>
          <dd>${escapeHtml(policyGateRuns.production_enforcement?.status || "unknown")} · blocked ${policyGateRuns.production_enforcement?.production_blocked ? "yes" : "no"} · ${escapeHtml(policyGateRuns.production_enforcement?.message || "not reported")}</dd>
        </dl>
        ${
          policyGateRunAttention.length
            ? `<table class="compact-table">
                <thead>
                  <tr>
                    <th>Severity</th>
                    <th>Signal</th>
                    <th>Message</th>
                  </tr>
                </thead>
                <tbody>
                  ${policyGateRunAttention
                    .map(
                      (item) => `
                        <tr>
                          <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                          <td>${escapeHtml(item.kind)}</td>
                          <td>${escapeHtml(item.message)}</td>
                        </tr>
                      `,
                    )
                    .join("")}
                </tbody>
              </table>`
            : `<div class="muted">No provider gate run attention items.</div>`
        }
        ${
          (policyGateRuns.recent_runs || []).length
            ? `<table class="compact-table">
                <thead>
                  <tr>
                    <th>Status</th>
                    <th>Ran at</th>
                    <th>Subject</th>
                    <th>Providers</th>
                    <th>Failed</th>
                    <th>Warnings</th>
                  </tr>
                </thead>
                <tbody>
                  ${(policyGateRuns.recent_runs || [])
                    .map(
                      (run) => `
                        <tr>
                          <td><span class="budget-status ${escapeHtml(run.status)}">${escapeHtml(run.status)}</span></td>
                          <td>${escapeHtml(run.ran_at)}</td>
                          <td>${escapeHtml(run.subject || "unknown")}</td>
                          <td>${formatInteger(run.provider_count)}</td>
                          <td>${escapeHtml((run.failed_provider_names || []).join("; ") || "none")}</td>
                          <td>${escapeHtml((run.warning_provider_names || []).join("; ") || "none")}</td>
                        </tr>
                      `,
                    )
                    .join("")}
                </tbody>
              </table>`
            : `<div class="muted">No provider policy gate runs.</div>`
        }
        <div class="muted">Generated: ${escapeHtml(policyGateRuns.generated_at)}</div>
      </div>
    `
    : "";
  const productionRolloutHtml = `
    <div class="detail-panel">
      <h4>Production Rollout</h4>
      ${
        productionRolloutRun
          ? `<div class="metric-grid compact-metrics">
              <div class="metric"><span>Status</span><strong>${escapeHtml(productionRolloutRun.status)}</strong></div>
              <div class="metric"><span>Providers</span><strong>${formatInteger(productionRolloutRun.provider_count)}</strong></div>
              <div class="metric"><span>Gate</span><strong>${escapeHtml(productionRolloutRun.enforcement?.status || "unknown")}</strong></div>
              <div class="metric"><span>Controller</span><strong>${escapeHtml(productionRolloutRun.controller_configured ? "configured" : "missing")}</strong></div>
            </div>
            <dl>
              <dt>Environment</dt>
              <dd>${escapeHtml(productionRolloutRun.environment || "production")}</dd>
              <dt>Controller execution</dt>
              <dd>${escapeHtml(productionRolloutRun.controller_execution?.status || "unknown")} · attempted ${escapeHtml(productionRolloutRun.controller_execution?.attempted ? "yes" : "no")} · HTTP ${escapeHtml(productionRolloutRun.controller_execution?.http_status || "n/a")}</dd>
              <dt>Message</dt>
              <dd>${escapeHtml(productionRolloutRun.message || "not reported")}</dd>
              <dt>Ran at</dt>
              <dd>${escapeHtml(productionRolloutRun.ran_at || "not recorded")}</dd>
            </dl>`
          : `<div class="muted">No production rollout run in this console session.</div>`
      }
    </div>
  `;
  const deploymentValidationHtml = `
    <div class="detail-panel">
      <h4>Deployment Validation</h4>
      <dl>
        <dt>Readiness</dt>
        <dd>${escapeHtml(deploymentReadiness.status || "unknown")} · blocked ${deploymentReadiness.production_blocked ? "yes" : "no"} · validated ${deploymentReadiness.deployment_validated ? "yes" : "no"} · healthy ${formatInteger(deploymentReadiness.healthy_count || 0)}/${formatInteger(deploymentReadiness.provider_count || 0)} · latest ${escapeHtml(deploymentReadiness.latest_validation_at || "none")}</dd>
        <dt>Controller</dt>
        <dd>required ${deploymentReadiness.controller_required ? "yes" : "no"} · configured ${deploymentReadiness.controller_configured ? "yes" : "no"} · latest ${escapeHtml(deploymentReadiness.latest_controller_status || "none")} · executions ${formatInteger(deploymentReadiness.controller_execution_count || 0)} · failed ${formatInteger(deploymentReadiness.controller_failed_count || 0)}</dd>
        <dt>Message</dt>
        <dd>${escapeHtml(deploymentReadiness.message || "provider deployment validation is not reported")}</dd>
      </dl>
      ${
        providerDeploymentValidation
          ? `<pre>${escapeHtml(JSON.stringify(providerDeploymentValidation, null, 2))}</pre>`
          : `<div class="muted">No provider deployment validation run in this console session.</div>`
      }
    </div>
  `;
  const summaryHtml = summary
    ? `
      <div class="detail-panel">
        <div class="metric-grid compact-metrics">
          <div class="metric"><span>Providers</span><strong>${formatInteger(summary.provider_count)}</strong></div>
          <div class="metric"><span>Active</span><strong>${formatInteger(summary.active_provider_count)}</strong></div>
          <div class="metric"><span>Pending</span><strong>${formatInteger(summary.pending_status_approval_count)}</strong></div>
          <div class="metric"><span>Emergency Changes</span><strong>${formatInteger(summary.emergency_lifecycle_count)}</strong></div>
          <div class="metric"><span>Vault Refs</span><strong>${formatInteger(summary.credential_ref_count)}</strong></div>
          <div class="metric"><span>Missing Keys</span><strong>${formatInteger(summary.missing_credential_count)}</strong></div>
          <div class="metric"><span>Budgeted</span><strong>${formatInteger(summary.budgeted_provider_count)}</strong></div>
          <div class="metric"><span>Attention</span><strong>${formatInteger(attentionItems.length)}</strong></div>
        </div>
        <div class="muted">Status: ${escapeHtml(formatCounts(summary.by_status))}</div>
        <div class="muted">Type: ${escapeHtml(formatCounts(summary.by_type))}</div>
        <div class="muted">Generated: ${escapeHtml(summary.generated_at)}</div>
        ${
          attentionItems.length
            ? `<table class="compact-table">
                <thead>
                  <tr>
                    <th>Severity</th>
                    <th>Provider</th>
                    <th>Signal</th>
                    <th>Message</th>
                  </tr>
                </thead>
                <tbody>
                  ${attentionItems
                    .map(
                      (item) => `
                        <tr>
                          <td><span class="budget-status ${escapeHtml(item.severity)}">${escapeHtml(item.severity)}</span></td>
                          <td>${escapeHtml(item.provider_name)}</td>
                          <td>${escapeHtml(item.kind)}</td>
                          <td>${escapeHtml(item.message)}</td>
                        </tr>
                      `,
                    )
                    .join("")}
                </tbody>
              </table>`
            : `<div class="muted">No provider governance attention items.</div>`
        }
      </div>
    `
    : "";
  const providerListHtml = state.providers.length
    ? state.providers
        .map(
          (provider) => {
            const health = state.providerHealth[provider.id];
            const pendingApproval = provider.config?.pending_status_approval;
            const lastApproval = provider.config?.last_status_approval;
            return `
            <div class="item">
              <strong>${escapeHtml(provider.name)}</strong>
              <div class="muted">${escapeHtml(provider.provider_type)} · ${escapeHtml(provider.status)}</div>
              <div class="muted">${escapeHtml(provider.id)} · ${escapeHtml(provider.default_model || "no default model")} · ${escapeHtml(provider.base_url || "no base URL")}</div>
              <button class="secondary" data-provider-status="${provider.id}" data-status="active">Activate</button>
              <button class="secondary reject" data-provider-status="${provider.id}" data-status="disabled">Disable</button>
              <button class="secondary reject" data-provider-status="${provider.id}" data-status="archived">Archive Provider</button>
              <button class="secondary" data-provider-health="${provider.id}">Check Health</button>
              ${
                pendingApproval
                  ? `<div class="item">
                      <strong>Pending provider approval</strong>
                      <div class="muted">${escapeHtml(pendingApproval.previous_status)} → ${escapeHtml(pendingApproval.requested_status)} · requested by ${escapeHtml(pendingApproval.requested_by || "unknown")}</div>
                      <div class="muted">Approver: ${escapeHtml(pendingApproval.approver_subject || "any different admin")} · ${escapeHtml(pendingApproval.reason || "no reason")}</div>
                      <button class="secondary" data-provider-approval="${provider.id}" data-decision="approve">Approve Provider Change</button>
                      <button class="secondary reject" data-provider-approval="${provider.id}" data-decision="reject">Reject Provider Change</button>
                    </div>`
                  : ""
              }
              ${
                lastApproval
                  ? `<div class="muted">Last provider approval: ${escapeHtml(lastApproval.status)} · ${escapeHtml(lastApproval.previous_status)} → ${escapeHtml(lastApproval.requested_status)} · ${escapeHtml(lastApproval.decided_at || "not decided")}</div>`
                  : ""
              }
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
  providerRoot.innerHTML =
    policyGateHtml + policyGateRunsHtml + deploymentValidationHtml + productionRolloutHtml + summaryHtml + providerListHtml;
  providerRoot.querySelectorAll("[data-provider-status]").forEach((button) => {
    button.addEventListener("click", () =>
      updateProviderStatus(button.dataset.providerStatus, button.dataset.status),
    );
  });
  providerRoot.querySelectorAll("[data-provider-health]").forEach((button) => {
    button.addEventListener("click", () => checkProviderHealth(button.dataset.providerHealth));
  });
  providerRoot.querySelectorAll("[data-provider-approval]").forEach((button) => {
    button.addEventListener("click", () =>
      decideProviderStatusApproval(button.dataset.providerApproval, button.dataset.decision),
    );
  });
}

async function updateProviderStatus(providerId, status) {
  await api(`/api/providers/${providerId}/status`, {
    method: "PATCH",
    body: JSON.stringify({
      status,
      emergency: true,
      reason: `Emergency provider ${status} from static console`,
    }),
  });
  await refreshOps();
}

async function decideProviderStatusApproval(providerId, decision) {
  await api(`/api/providers/${providerId}/status-approval/${decision}`, {
    method: "POST",
    body: JSON.stringify({ comment: `Provider change ${decision} from static console` }),
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

async function runVaultKmsRotation() {
  state.vaultKmsRotationRun = await api("/api/vault/kms/rotation/run", {
    method: "POST",
  });
  await refreshOps();
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

function renderVaultReadiness() {
  const readiness = state.vaultReadiness;
  if (!readiness) {
    vaultReadinessRoot.innerHTML = `<div class="muted">Vault readiness data is not loaded.</div>`;
    return;
  }
  const attentionItems = readiness.attention_items || [];
  const checks = readiness.checks || [];
  const productionRotation = readiness.production_rotation || {};
  const kmsRotationRun = state.vaultKmsRotationRun;
  vaultReadinessRoot.innerHTML = `
    <div class="metric-grid">
      <div class="metric"><span>Readiness</span><strong>${escapeHtml(readiness.status)}</strong></div>
      <div class="metric"><span>Secret Refs</span><strong>${formatInteger(readiness.active_secret_record_count)}/${formatInteger(readiness.secret_record_count)}</strong></div>
      <div class="metric"><span>Consumers</span><strong>${formatInteger(readiness.provider_ref_count + readiness.mcp_secret_ref_count + readiness.eval_judge_secret_ref_count)}</strong></div>
      <div class="metric"><span>Unresolved</span><strong>${formatInteger(readiness.unresolved_ref_count)}</strong></div>
      <div class="metric"><span>Stale Rotations</span><strong>${formatInteger(readiness.stale_rotation_count)}</strong></div>
      <div class="metric"><span>KMS</span><strong>${escapeHtml(readiness.kms?.status || "unknown")}</strong></div>
      <div class="metric"><span>Prod Rotation</span><strong>${escapeHtml(productionRotation.status || "unknown")}</strong></div>
    </div>
    <div class="item">
      <strong>Secret provider: ${escapeHtml(readiness.secret_provider?.status || "unknown")}</strong>
      <div class="muted">${escapeHtml(readiness.secret_provider?.provider_kind || "unknown")} · ${escapeHtml(readiness.secret_provider?.healthy ? "healthy" : "unhealthy")}</div>
      <div class="muted">KMS ${escapeHtml(readiness.kms?.provider || "reserved")} · key ${escapeHtml(readiness.kms?.key_id_configured ? "configured" : "missing")} · rotation ${escapeHtml(readiness.kms?.rotation_policy_configured ? "configured" : "missing")} · endpoint ${escapeHtml(readiness.kms?.endpoint_configured ? "configured" : "missing")} · validation ${escapeHtml(readiness.kms?.validation_mode || "health-check")}</div>
    </div>
    <div class="item">
      <strong>PRODUCTION ROTATION GATE</strong>
      <div class="muted">${escapeHtml(productionRotation.message || "Vault production rotation gate is not reported")}</div>
      <div class="muted">Vault ${productionRotation.vault_healthy ? "healthy" : "not ready"} · KMS ${productionRotation.kms_ready ? "ready" : "not ready"} · refs ${productionRotation.unresolved_refs_clear ? "resolved" : "unresolved"} · stale ${productionRotation.stale_rotations_clear ? "clear" : "present"} · latest run ${escapeHtml(productionRotation.latest_rotation_run_status || "none")}</div>
    </div>
    <div class="item">
      <strong>KMS ROTATION GATE</strong>
      ${
        kmsRotationRun
          ? `<div class="muted">${escapeHtml(kmsRotationRun.status)} · KMS ${escapeHtml(kmsRotationRun.kms_status)} · endpoint ${escapeHtml(kmsRotationRun.kms_endpoint_configured ? "configured" : "missing")} · secret provider ${escapeHtml(kmsRotationRun.secret_provider_status)} · stale refs ${formatInteger(kmsRotationRun.stale_rotation_count || 0)} · rotated ${formatInteger(kmsRotationRun.rotated_count || 0)} · catalog updates ${formatInteger(kmsRotationRun.catalog_updated_count || 0)}</div>
             <div class="muted">External execution: ${escapeHtml(kmsRotationRun.external_execution?.status || "unknown")} · attempted ${escapeHtml(kmsRotationRun.external_execution?.attempted ? "yes" : "no")} · HTTP ${escapeHtml(kmsRotationRun.external_execution?.http_status || "n/a")}</div>
             <pre>${escapeHtml(JSON.stringify(kmsRotationRun, null, 2))}</pre>`
          : `<div class="muted">No KMS rotation gate run in this console session</div>`
      }
    </div>
    ${
      attentionItems.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Severity</th>
                <th>Resource</th>
                <th>Issue</th>
              </tr>
            </thead>
            <tbody>
              ${attentionItems
                .slice(0, 8)
                .map(
                  (item) => `
                    <tr>
                      <td>${escapeHtml(item.severity)}</td>
                      <td>${escapeHtml(item.resource_type)} · ${escapeHtml(item.resource_name)}</td>
                      <td>${escapeHtml(item.message)}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No Vault readiness attention items.</div>`
    }
    ${
      checks.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Check</th>
                <th>Status</th>
                <th>Refs</th>
                <th>Next action</th>
              </tr>
            </thead>
            <tbody>
              ${checks
                .slice(0, 12)
                .map(
                  (check) => `
                    <tr>
                      <td>${escapeHtml(check.resource_type)} · ${escapeHtml(check.resource_name)}</td>
                      <td>${escapeHtml(check.status)}</td>
                      <td>${escapeHtml((check.secret_refs || []).join(", ") || "none")}</td>
                      <td>${escapeHtml((check.recommendations || []).join(" · ") || (check.blockers || []).join(" · ") || (check.warnings || []).join(" · ") || "No action")}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : ""
    }
  `;
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
  renderApprovalNotificationRouting();
  const dueRun = state.approvalEscalationDueRun
    ? `<div class="item">
        <strong>Due escalation run: ${escapeHtml(state.approvalEscalationDueRun.status)}</strong>
        <div class="muted">${formatInteger(state.approvalEscalationDueRun.escalated_count)} escalated · ${formatInteger(state.approvalEscalationDueRun.expired_count)} expired · ${formatInteger(state.approvalEscalationDueRun.skipped_count)} skipped · ${escapeHtml(state.approvalEscalationDueRun.checked_at)}</div>
      </div>`
    : "";
  approvalGovernanceRoot.innerHTML = `
    ${dueRun}
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

function renderApprovalNotificationRouting() {
  const routing = state.approvalNotificationRouting;
  if (!routing) {
    approvalNotificationRoutingRoot.innerHTML = `<div class="muted">Approval notification routing is not loaded.</div>`;
    return;
  }
  const attentionItems = routing.attention_items || [];
  approvalNotificationRoutingRoot.innerHTML = `
    <div class="metric-grid">
      <div class="metric"><span>Routing</span><strong>${escapeHtml(routing.status)}</strong></div>
      <div class="metric"><span>Channels</span><strong>${formatInteger(routing.channel_count)}</strong></div>
      <div class="metric"><span>Policies</span><strong>${formatInteger(routing.active_policy_count)} / ${formatInteger(routing.persisted_policy_count)}</strong></div>
      <div class="metric"><span>Pending</span><strong>${formatInteger(routing.pending_approval_count)}</strong></div>
      <div class="metric"><span>Routable</span><strong>${formatInteger(routing.routable_pending_count)}</strong></div>
      <div class="metric"><span>Unroutable</span><strong>${formatInteger(routing.unroutable_pending_count)}</strong></div>
      <div class="metric"><span>Groups</span><strong>${formatInteger(routing.approval_group_count)}</strong></div>
    </div>
    <div class="item">
      <strong>Channels</strong>
      <div class="muted">webhook ${escapeHtml(routing.webhook_configured ? "configured" : "missing")} · slack ${escapeHtml(routing.slack_configured ? "configured" : "missing")} · email relay ${escapeHtml(routing.email_relay_configured ? "configured" : "missing")}</div>
      <div class="muted">${formatInteger(routing.delegated_pending_count)} delegated pending · ${formatInteger(routing.group_pending_count)} group-routed pending · ${formatInteger(routing.escalation_rule_count)} escalation rules</div>
    </div>
    ${
      (routing.channel_policies || []).length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Policy</th>
                <th>Channel</th>
                <th>Risk</th>
                <th>Retry</th>
                <th>Target Env</th>
                <th>Status</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              ${(routing.channel_policies || [])
                .slice(0, 8)
                .map(
                  (policy) => `
                    <tr>
                      <td>${escapeHtml(policy.name)}</td>
                      <td>${escapeHtml(policy.channel)}</td>
                      <td>${escapeHtml(policy.risk_filter)}</td>
                      <td>${formatInteger(policy.max_attempts)} attempts · ${formatInteger(policy.backoff_seconds)}s</td>
                      <td>${escapeHtml(policy.target_env || "default env boundary")}</td>
                      <td>${escapeHtml(policy.status)}</td>
                      <td>${
                        policy.status === "active"
                          ? `<button type="button" class="secondary" data-archive-approval-notification-policy="${escapeHtml(policy.id)}">Archive</button>`
                          : `<span class="muted">No action</span>`
                      }</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No persisted approval notification channel policies.</div>`
    }
    ${
      attentionItems.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Severity</th>
                <th>Kind</th>
                <th>Approval</th>
                <th>Message</th>
              </tr>
            </thead>
            <tbody>
              ${attentionItems
                .slice(0, 8)
                .map(
                  (item) => `
                    <tr>
                      <td>${escapeHtml(item.severity)}</td>
                      <td>${escapeHtml(item.kind)}</td>
                      <td>${escapeHtml(item.approval_id || "global")}</td>
                      <td>${escapeHtml(item.message)}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No approval notification routing attention items.</div>`
    }
  `;
  approvalNotificationRoutingRoot
    .querySelectorAll("[data-archive-approval-notification-policy]")
    .forEach((button) => {
      button.addEventListener("click", () =>
        archiveApprovalNotificationChannelPolicy(
          button.dataset.archiveApprovalNotificationPolicy,
        ),
      );
    });
}

function renderApprovalNotificationRuns() {
  const runs = state.approvalNotificationRuns;
  const latestRun = state.approvalNotificationRun;
  if (!runs) {
    approvalNotificationRunsRoot.innerHTML = `<div class="muted">Approval notification runs are not loaded.</div>`;
    return;
  }
  const attentionItems = runs.attention_items || [];
  const recentRuns = runs.recent_runs || [];
  const productionOps = runs.production_ops || {};
  const deploymentReadiness = runs.deployment_readiness || {};
  const deploymentValidation = state.approvalNotificationDeploymentValidation;
  approvalNotificationRunsRoot.innerHTML = `
    <div class="item">
      <strong>NOTIFICATION RUNS</strong>
      <div class="muted">Runs ${formatInteger(runs.run_count)} · delivered ${formatInteger(runs.delivered_run_count)} · reserved ${formatInteger(runs.reserved_run_count)} · failed ${formatInteger(runs.failed_run_count)}</div>
      <div class="muted">Production ops: ${escapeHtml(productionOps.status || "unknown")} · blocked ${productionOps.production_blocked ? "yes" : "no"} · routing ${escapeHtml(productionOps.routing_status || "unknown")} · channels ${formatInteger(productionOps.channel_count || 0)} · unroutable ${formatInteger(productionOps.unroutable_pending_count || 0)}</div>
      <div class="muted">${escapeHtml(productionOps.message || "Production approval notification ops are not reported")}</div>
      <div class="muted">Deployment validation: ${escapeHtml(deploymentReadiness.status || "unknown")} · blocked ${deploymentReadiness.production_blocked ? "yes" : "no"} · channels ${formatInteger(deploymentReadiness.channel_count || 0)} · policies ${formatInteger(deploymentReadiness.active_policy_count || 0)} / ${formatInteger(deploymentReadiness.persisted_policy_count || 0)} · unroutable ${formatInteger(deploymentReadiness.unroutable_pending_count || 0)}</div>
      <div class="muted">Deployment controller: required ${deploymentReadiness.controller_required ? "yes" : "no"} · configured ${deploymentReadiness.controller_configured ? "yes" : "no"} · latest ${escapeHtml(deploymentReadiness.latest_controller_status || "none")} · executions ${formatInteger(deploymentReadiness.controller_execution_count || 0)} · failed ${formatInteger(deploymentReadiness.controller_failed_count || 0)}</div>
      <div class="muted">${escapeHtml(deploymentReadiness.message || "Approval notification deployment validation has not been reported")}</div>
      ${
        latestRun
          ? `<div class="muted">Latest action: ${escapeHtml(latestRun.status)} · delivered ${formatInteger(latestRun.delivered_count)} · reserved ${formatInteger(latestRun.reserved_count)} · failed ${formatInteger(latestRun.failed_count)} · skipped ${formatInteger(latestRun.skipped_count)}</div>`
          : `<div class="muted">No notification run triggered in this browser session</div>`
      }
      ${
        deploymentValidation
          ? `<div class="muted">Latest validation: ${escapeHtml(deploymentValidation.status)} · pending ${formatInteger(deploymentValidation.pending_approval_count)} · routable ${formatInteger(deploymentValidation.routable_pending_count)} · unroutable ${formatInteger(deploymentValidation.unroutable_pending_count)} · controller ${escapeHtml(deploymentValidation.controller_execution?.status || "skipped")} · checked ${escapeHtml(deploymentValidation.checked_at)}</div>`
          : `<div class="muted">No notification deployment validation triggered in this browser session</div>`
      }
    </div>
    ${
      recentRuns.length
        ? `<table class="usage-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Delivered</th>
                <th>Reserved</th>
                <th>Failed</th>
                <th>Skipped</th>
                <th>Ran</th>
              </tr>
            </thead>
            <tbody>
              ${recentRuns
                .slice(0, 5)
                .map(
                  (run) => `
                    <tr>
                      <td>${escapeHtml(run.status)}</td>
                      <td>${formatInteger(run.delivered_count)}</td>
                      <td>${formatInteger(run.reserved_count)}</td>
                      <td>${formatInteger(run.failed_count)}</td>
                      <td>${formatInteger(run.skipped_count)}</td>
                      <td>${escapeHtml(run.ran_at)}</td>
                    </tr>
                  `,
                )
                .join("")}
            </tbody>
          </table>`
        : `<div class="muted">No approval notification runs</div>`
    }
    ${
      attentionItems.length
        ? attentionItems
            .map(
              (item) =>
                `<div class="muted">${escapeHtml(item.severity)} · ${escapeHtml(item.kind)} · ${escapeHtml(item.message)}</div>`,
            )
            .join("")
        : `<div class="muted">No approval notification delivery attention items.</div>`
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
              <button class="secondary" data-eval-request-prod="${run.id}">Request Prod Approval</button>
              <button class="secondary" data-eval-request-auto-prod="${run.id}">Request Auto Prod</button>
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
  evalRunRoot.querySelectorAll("[data-eval-request-prod]").forEach((button) => {
    button.addEventListener("click", () => requestEvalRunPromotion(button.dataset.evalRequestProd));
  });
  evalRunRoot.querySelectorAll("[data-eval-request-auto-prod]").forEach((button) => {
    button.addEventListener("click", () =>
      requestEvalRunAutoPromotion(button.dataset.evalRequestAutoProd),
    );
  });
  evalRunRoot.querySelectorAll("[data-eval-promote-prod]").forEach((button) => {
    button.addEventListener("click", () =>
      promoteEvalRun(button.dataset.evalPromoteProd, "prod"),
    );
  });
}

function renderEvalJudgeProfiles() {
  evalJudgeProfileRoot.innerHTML = state.evalJudgeProfiles.length
    ? state.evalJudgeProfiles
        .map(
          (profile) => `
            <div class="item">
              <strong>${escapeHtml(profile.name)}</strong>
              <div class="muted">${escapeHtml(profile.status)} · ${escapeHtml(profile.default_model || "no model")}</div>
              <div class="muted">${escapeHtml(profile.base_url || "no endpoint")}</div>
              <div class="muted">API key ref: ${escapeHtml(profile.config?.api_key_ref ? "configured" : "not configured")}</div>
              <pre>${escapeHtml(
                JSON.stringify(
                  {
                    provider_type: profile.provider_type,
                    timeout_seconds: profile.config?.timeout_seconds,
                    grading_policy: {
                      kind: "judge",
                      judge_profile: profile.name,
                    },
                  },
                  null,
                  2,
                ),
              )}</pre>
            </div>
          `,
        )
        .join("")
    : `<div class="muted">No eval judge profiles</div>`;
}

function renderEvalSuiteBootstrap() {
  evalSuiteBootstrapRoot.innerHTML = state.evalSuiteBootstrap
    ? `<div class="item">
        <strong>Stage 2 suite bootstrapped</strong>
        <div class="muted">${escapeHtml(state.evalSuiteBootstrap.dataset.name)} · ${escapeHtml(state.evalSuiteBootstrap.dataset.id)}</div>
        <div class="muted">${formatInteger(state.evalSuiteBootstrap.cases.length)} eval cases created</div>
      </div>`
    : `<div class="muted">No bootstrapped eval suite yet.</div>`;
}

function renderAgentReleases() {
  const summary = state.agentReleaseSummary;
  const summaryPanel = summary
    ? `<div class="policy-gate-summary">
        <div class="metric-grid compact-metrics">
          <div class="metric"><span>Total</span><strong>${formatInteger(summary.release_count)}</strong></div>
          <div class="metric"><span>Pending</span><strong>${formatInteger(summary.pending_count)}</strong></div>
          <div class="metric"><span>Promoted</span><strong>${formatInteger(summary.promoted_count)}</strong></div>
          <div class="metric"><span>Rejected</span><strong>${formatInteger(summary.rejected_count)}</strong></div>
          <div class="metric"><span>Rolled Back</span><strong>${formatInteger(summary.rolled_back_count)}</strong></div>
          <div class="metric"><span>Auto Pending</span><strong>${formatInteger(summary.auto_pending_count)}</strong></div>
          <div class="metric"><span>Expired</span><strong>${formatInteger(summary.expired_pending_count)}</strong></div>
          <div class="metric"><span>Stale</span><strong>${formatInteger(summary.stale_pending_count)}</strong></div>
        </div>
        <div class="muted">Status: ${escapeHtml(formatCounts(summary.by_status))}</div>
        <div class="muted">Environment: ${escapeHtml(formatCounts(summary.by_environment))}</div>
        <div class="muted">Generated: ${escapeHtml(summary.generated_at)}</div>
        ${renderReleaseAttention(summary.attention_items || [])}
        ${renderLatestPromotions(summary.latest_promoted_by_environment || [])}
      </div>`
    : "";
  const automationRun = state.agentReleaseAutomationRun;
  const automationSummary = automationRun
    ? `<div class="item">
        <strong>Release automation run</strong>
        <div class="muted">${formatInteger(automationRun.pending_count)} pending · ${formatInteger(automationRun.promoted_count)} promoted · ${formatInteger(automationRun.rejected_count)} rejected · ${formatInteger(automationRun.skipped_count)} skipped · ${escapeHtml(automationRun.checked_at)}</div>
        <div class="muted">Controller ${escapeHtml(automationRun.controller_configured ? "configured" : "not configured")} · required ${automationRun.controller_required ? "yes" : "no"} · executions ${formatInteger(automationRun.controller_execution_count || 0)} · failed ${formatInteger(automationRun.controller_failed_count || 0)}</div>
      </div>`
    : "";
  const automationRunHistory = renderAgentReleaseAutomationRuns(
    state.agentReleaseAutomationRuns,
  );
  const releaseGroups = state.agents
    .map((agent) => ({
      agent,
      releases: state.agentReleases[agent.id] || [],
    }))
    .filter((group) => group.releases.length);
  agentReleaseRoot.innerHTML = releaseGroups.length
    ? `${summaryPanel}${automationSummary}${automationRunHistory}${releaseGroups
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
                      <div class="muted">Requested by: ${escapeHtml(release.requested_by || "none")} · Approver: ${escapeHtml(release.approver_subject || "any separate admin")}</div>
                      <div class="muted">Automation: ${escapeHtml(release.automation_policy?.auto_approve ? "auto approve" : "manual")} · after ${escapeHtml(release.automation_policy?.activate_after || "n/a")} · expires ${escapeHtml(release.automation_policy?.expires_at || "n/a")}</div>
                      <div class="muted">Decision by: ${escapeHtml(release.decision_by || "pending")} · Reason: ${escapeHtml(release.decision_reason || release.request_reason || "none")}</div>
                      ${
                        release.status === "pending_approval"
                          ? `<button class="secondary" data-release-approve="${release.id}" data-release-agent="${agent.id}">Approve</button>
                             <button class="secondary" data-release-reject="${release.id}" data-release-agent="${agent.id}">Reject</button>`
                          : ""
                      }
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
        .join("")}`
    : `${summaryPanel}${automationSummary}${automationRunHistory}<div class="muted">No promoted or rolled back releases</div>`;
  agentReleaseRoot.querySelectorAll("[data-release-rollback]").forEach((button) => {
    button.addEventListener("click", () =>
      rollbackAgentRelease(button.dataset.releaseAgent, button.dataset.releaseRollback),
    );
  });
  agentReleaseRoot.querySelectorAll("[data-release-approve]").forEach((button) => {
    button.addEventListener("click", () =>
      approveAgentRelease(button.dataset.releaseAgent, button.dataset.releaseApprove),
    );
  });
  agentReleaseRoot.querySelectorAll("[data-release-reject]").forEach((button) => {
    button.addEventListener("click", () =>
      rejectAgentRelease(button.dataset.releaseAgent, button.dataset.releaseReject),
    );
  });
}

function renderAgentReleaseAutomationRuns(runs) {
  if (!runs) {
    return `<div class="muted">Release automation runs are not loaded.</div>`;
  }
  const recentRuns = runs.recent_runs || [];
  const attentionItems = runs.attention_items || [];
  const productionOps = runs.production_ops || {};
  const productionOrchestration = runs.production_orchestration || {};
  return `
    <div class="nested-item">
      <strong>RELEASE AUTOMATION RUNS</strong>
      <div class="muted">Runs ${formatInteger(runs.run_count)} · processed ${formatInteger(runs.processed_run_count)} · skipped ${formatInteger(runs.skipped_run_count)}</div>
      <div class="muted">Production rollout: ${escapeHtml(productionOps.status || "unknown")} · blocked ${productionOps.production_blocked ? "yes" : "no"} · pending ${formatInteger(productionOps.pending_count || 0)} · auto ${formatInteger(productionOps.auto_pending_count || 0)} · manual ${formatInteger(productionOps.manual_pending_count || 0)}</div>
      <div class="muted">${escapeHtml(productionOps.message || "Release production ops are not reported")}</div>
      <div class="muted">Production orchestration: ${escapeHtml(productionOrchestration.status || "unknown")} · supervision fresh ${productionOrchestration.automation_supervision_fresh ? "yes" : "no"} · pending clear ${productionOrchestration.pending_clear ? "yes" : "no"} · skipped clear ${productionOrchestration.skipped_automation_clear ? "yes" : "no"} · manual clear ${productionOrchestration.manual_approval_clear ? "yes" : "no"}</div>
      <div class="muted">${escapeHtml(productionOrchestration.message || "Release production orchestration is not reported")}</div>
      ${
        recentRuns.length
          ? `<table class="usage-table">
              <thead>
                <tr>
                  <th>Status</th>
                  <th>Pending</th>
                  <th>Promoted</th>
                  <th>Rejected</th>
                  <th>Skipped</th>
                  <th>Ran</th>
                </tr>
              </thead>
              <tbody>
                ${recentRuns
                  .slice(0, 5)
                  .map(
                    (run) => `
                      <tr>
                        <td>${escapeHtml(run.status)}</td>
                        <td>${formatInteger(run.pending_count)}</td>
                        <td>${formatInteger(run.promoted_count)}</td>
                        <td>${formatInteger(run.rejected_count)}</td>
                        <td>${formatInteger(run.skipped_count)}</td>
                        <td>${escapeHtml(run.ran_at)}</td>
                      </tr>
                    `,
                  )
                  .join("")}
              </tbody>
            </table>`
          : `<div class="muted">No release automation runs</div>`
      }
      ${
        attentionItems.length
          ? attentionItems
              .map(
                (item) =>
                  `<div class="muted">${escapeHtml(item.severity)} · ${escapeHtml(item.kind)} · ${escapeHtml(item.message)}</div>`,
              )
              .join("")
          : `<div class="muted">No release automation run attention items.</div>`
      }
    </div>
  `;
}

function renderReleaseAttention(items) {
  return items.length
    ? `<div class="nested-item">
        <strong>Release attention</strong>
        <table class="usage-table">
          <thead>
            <tr><th>Environment</th><th>Status</th><th>Reason</th><th>Approver</th><th>Expires</th></tr>
          </thead>
          <tbody>
            ${items
              .map(
                (item) => `
                  <tr>
                    <td>${escapeHtml(item.environment)}</td>
                    <td>${escapeHtml(item.status)}</td>
                    <td>${escapeHtml(item.reason || "review")}</td>
                    <td>${escapeHtml(item.approver_subject || "any separate admin")}</td>
                    <td>${escapeHtml(item.expires_at || "none")}</td>
                  </tr>
                `,
              )
              .join("")}
          </tbody>
        </table>
      </div>`
    : `<div class="muted">No pending release attention items.</div>`;
}

function renderLatestPromotions(items) {
  return items.length
    ? `<div class="nested-item">
        <strong>Latest promoted by environment</strong>
        ${items
          .map(
            (item) => `
              <div class="muted">${escapeHtml(item.environment)} · ${escapeHtml(item.release_id)} · score ${escapeHtml(item.eval_score ?? "n/a")} · ${escapeHtml(item.promoted_at)}</div>
            `,
          )
          .join("")}
      </div>`
    : `<div class="muted">No promoted release per environment yet.</div>`;
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
  organizationOwnerForm.elements.organization_id.value = organizationId;
  teamForm.elements.organization_id.value = organizationId;
  membershipForm.elements.organization_id.value = organizationId;
  tenantInvitationForm.elements.organization_id.value = organizationId;
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
  const rolloutRun = state.mcpRolloutRun;
  const rolloutRunSummary = rolloutRun
    ? `<div class="item">
        <strong>Due rollout run</strong>
        <div class="muted">${formatInteger(rolloutRun.applied_count)} applied · ${formatInteger(rolloutRun.skipped_count)} skipped · ${formatInteger(rolloutRun.expired_count)} expired · ${formatInteger(rolloutRun.failed_count)} failed · ${escapeHtml(rolloutRun.checked_at)}</div>
      </div>`
    : "";
  const deploymentValidation = state.mcpDeploymentValidation;
  const deploymentValidationSummary = deploymentValidation
    ? `<div class="item">
        <strong>Deployment validation</strong>
        <div class="muted">${escapeHtml(deploymentValidation.status || "unknown")} · ${formatInteger(deploymentValidation.healthy_count)} healthy · ${formatInteger(deploymentValidation.unhealthy_count)} unhealthy · ${formatInteger(deploymentValidation.server_count)} servers · ${escapeHtml(deploymentValidation.checked_at)}</div>
      </div>`
    : "";
  const rolloutSummary = renderMcpRolloutSummary(state.mcpRolloutSummary);
  const rolloutRuns = renderMcpRolloutRuns(state.mcpRolloutRuns);
  mcpServerRoot.innerHTML = state.mcpServers.length
    ? `${rolloutSummary}${rolloutRuns}${runSummary}${scheduledRunSummary}${deploymentValidationSummary}${rolloutRunSummary}${state.mcpServers
        .map((server) => {
          const health = state.mcpHealth[server.id];
          const pendingRollout = server.config?.pending_rollout;
          const lastRollout = server.config?.last_rollout;
          return `
            <div class="item">
              <strong>${escapeHtml(server.name)}</strong>
              <div class="muted">${escapeHtml(server.transport)} · ${escapeHtml(server.status)}</div>
              <div class="muted">Tools: ${escapeHtml(server.tool_allowlist.join(", ") || "none")}</div>
              <div class="muted">Secret refs: ${escapeHtml((server.config?.secret_refs || []).join(", ") || "none")}</div>
              <div class="muted">Rollout: ${escapeHtml(pendingRollout ? `pending ${pendingRollout.id}` : lastRollout ? `${lastRollout.status} ${lastRollout.id}` : "none")}</div>
              <button class="secondary" data-edit-mcp="${server.id}">Edit Config</button>
              <button class="secondary" data-rollout-mcp="${server.id}">Request Rollout</button>
              ${pendingRollout ? `<button class="secondary" data-apply-mcp-rollout="${server.id}" data-rollout-id="${escapeHtml(pendingRollout.id)}">Apply Rollout</button>` : ""}
              ${lastRollout?.status === "applied" ? `<button class="secondary" data-rollback-mcp-rollout="${server.id}" data-rollout-id="${escapeHtml(lastRollout.id)}">Rollback Rollout</button>` : ""}
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
      ? `${rolloutSummary}${rolloutRuns}${runSummary}${scheduledRunSummary}${deploymentValidationSummary}${rolloutRunSummary}<div class="muted">No MCP servers for this team</div>`
      : `<div class="muted">MCP ROLLOUT RUNS require a team ID</div><div class="muted">Enter a team ID to manage MCP servers</div>`;
  mcpServerRoot.querySelectorAll("[data-discover-mcp]").forEach((button) => {
    button.addEventListener("click", () => discoverMcpTools(button.dataset.discoverMcp));
  });
  mcpServerRoot.querySelectorAll("[data-edit-mcp]").forEach((button) => {
    button.addEventListener("click", () => editMcpServer(button.dataset.editMcp));
  });
  mcpServerRoot.querySelectorAll("[data-rollout-mcp]").forEach((button) => {
    button.addEventListener("click", () => requestMcpRollout(button.dataset.rolloutMcp));
  });
  mcpServerRoot.querySelectorAll("[data-apply-mcp-rollout]").forEach((button) => {
    button.addEventListener("click", () =>
      applyMcpRollout(button.dataset.applyMcpRollout, button.dataset.rolloutId),
    );
  });
  mcpServerRoot.querySelectorAll("[data-rollback-mcp-rollout]").forEach((button) => {
    button.addEventListener("click", () =>
      rollbackMcpRollout(button.dataset.rollbackMcpRollout, button.dataset.rolloutId),
    );
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

function renderMcpRolloutRuns(runs) {
  if (!runs) {
    return "";
  }
  const recentRuns = runs.recent_runs || [];
  const attentionItems = runs.attention_items || [];
  const productionOps = runs.production_ops || {};
  const productionOrchestration = runs.production_orchestration || {};
  const deploymentReadiness = runs.deployment_readiness || {};
  return `
    <div class="nested-item">
      <strong>MCP ROLLOUT RUNS</strong>
      <div class="muted">Runs ${formatInteger(runs.run_count)} · processed ${formatInteger(runs.processed_run_count)} · failed ${formatInteger(runs.failed_run_count)}</div>
      <div class="muted">Latest controller ${runs.latest_run?.controller_configured ? "configured" : "not configured"} · required ${runs.latest_run?.controller_required ? "yes" : "no"} · executions ${formatInteger(runs.latest_run?.controller_execution_count || 0)} · failed ${formatInteger(runs.latest_run?.controller_failed_count || 0)}</div>
      <div class="muted">Production rollout: ${escapeHtml(productionOps.status || "unknown")} · blocked ${productionOps.production_blocked ? "yes" : "no"} · pending ${formatInteger(productionOps.pending_rollout_count || 0)} · due ${formatInteger(productionOps.due_pending_count || 0)} · failed preflight ${formatInteger(productionOps.failed_preflight_count || 0)}</div>
      <div class="muted">${escapeHtml(productionOps.message || "MCP production rollout ops are not reported")}</div>
      <div class="muted">Production orchestration: ${escapeHtml(productionOrchestration.status || "unknown")} · scheduler fresh ${productionOrchestration.scheduler_supervision_fresh ? "yes" : "no"} · pending clear ${productionOrchestration.pending_clear ? "yes" : "no"} · failed runs clear ${productionOrchestration.failed_runs_clear ? "yes" : "no"} · manual apply ${formatInteger(productionOrchestration.manual_apply_required_count || 0)}</div>
      <div class="muted">${escapeHtml(productionOrchestration.message || "MCP production orchestration is not reported")}</div>
      <div class="muted">Deployment validation: ${escapeHtml(deploymentReadiness.status || "unknown")} · blocked ${deploymentReadiness.production_blocked ? "yes" : "no"} · validated ${deploymentReadiness.deployment_validated ? "yes" : "no"} · healthy ${formatInteger(deploymentReadiness.healthy_count || 0)}/${formatInteger(deploymentReadiness.server_count || 0)} · latest ${escapeHtml(deploymentReadiness.latest_validation_at || "none")}</div>
      <div class="muted">${escapeHtml(deploymentReadiness.message || "MCP deployment validation is not reported")}</div>
      ${
        recentRuns.length
          ? `<table class="usage-table">
              <thead>
                <tr>
                  <th>Status</th>
                  <th>Applied</th>
                  <th>Expired</th>
                  <th>Failed</th>
                  <th>Skipped</th>
                  <th>Ran</th>
                </tr>
              </thead>
              <tbody>
                ${recentRuns
                  .slice(0, 5)
                  .map(
                    (run) => `
                      <tr>
                        <td>${escapeHtml(run.status)}</td>
                        <td>${formatInteger(run.applied_count)}</td>
                        <td>${formatInteger(run.expired_count)}</td>
                        <td>${formatInteger(run.failed_count)}</td>
                        <td>${formatInteger(run.skipped_count)}</td>
                        <td>${escapeHtml(run.ran_at)}</td>
                      </tr>
                    `,
                  )
                  .join("")}
              </tbody>
            </table>`
          : `<div class="muted">No MCP rollout runs</div>`
      }
      ${
        attentionItems.length
          ? attentionItems
              .map(
                (item) =>
                  `<div class="muted">${escapeHtml(item.severity)} · ${escapeHtml(item.kind)} · ${escapeHtml(item.message)}</div>`,
              )
              .join("")
          : `<div class="muted">No MCP rollout run attention items.</div>`
      }
    </div>
  `;
}

function renderMcpRolloutSummary(summary) {
  return summary
    ? `<div class="policy-gate-summary">
        <div class="metric-grid compact-metrics">
          <div class="metric"><span>Servers</span><strong>${formatInteger(summary.server_count)}</strong></div>
          <div class="metric"><span>Pending</span><strong>${formatInteger(summary.pending_rollout_count)}</strong></div>
          <div class="metric"><span>Due</span><strong>${formatInteger(summary.due_pending_count)}</strong></div>
          <div class="metric"><span>Not Due</span><strong>${formatInteger(summary.not_due_pending_count)}</strong></div>
          <div class="metric"><span>Expired</span><strong>${formatInteger(summary.expired_pending_count)}</strong></div>
          <div class="metric"><span>Applied</span><strong>${formatInteger(summary.applied_rollout_count)}</strong></div>
          <div class="metric"><span>Rolled Back</span><strong>${formatInteger(summary.rolled_back_rollout_count)}</strong></div>
          <div class="metric"><span>Preflight Failed</span><strong>${formatInteger(summary.failed_preflight_count)}</strong></div>
        </div>
        <div class="muted">Server status: ${escapeHtml(formatCounts(summary.by_server_status))}</div>
        <div class="muted">Transport: ${escapeHtml(formatCounts(summary.by_transport))}</div>
        <div class="muted">Generated: ${escapeHtml(summary.generated_at)}</div>
        ${renderMcpRolloutAttention(summary.attention_items || [])}
        ${renderMcpLatestRollouts(summary.latest_rollouts || [])}
      </div>`
    : "";
}

function renderMcpRolloutAttention(items) {
  return items.length
    ? `<div class="nested-item">
        <strong>Connector rollout attention</strong>
        <table class="usage-table">
          <thead>
            <tr><th>Server</th><th>Status</th><th>Reason</th><th>Targets</th><th>Window</th></tr>
          </thead>
          <tbody>
            ${items
              .map(
                (item) => `
                  <tr>
                    <td>${escapeHtml(item.name)}</td>
                    <td>${escapeHtml(item.rollout_status)}</td>
                    <td>${escapeHtml(item.reason || "review")}</td>
                    <td>${escapeHtml((item.target_keys || []).join(", ") || "none")}</td>
                    <td>${escapeHtml(item.activate_after || "manual")} -> ${escapeHtml(item.activate_before || "open")}</td>
                  </tr>
                `,
              )
              .join("")}
          </tbody>
        </table>
      </div>`
    : `<div class="muted">No pending connector rollout attention items.</div>`;
}

function renderMcpLatestRollouts(items) {
  return items.length
    ? `<div class="nested-item">
        <strong>Latest connector rollouts</strong>
        ${items
          .map(
            (item) => `
              <div class="muted">${escapeHtml(item.name)} · ${escapeHtml(item.status)} · ${escapeHtml(item.rollout_id || "no id")} · ${escapeHtml(item.updated_at || "no timestamp")}</div>
            `,
          )
          .join("")}
      </div>`
    : `<div class="muted">No applied connector rollouts yet.</div>`;
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
          ? `<div class="muted">Delivery: ${escapeHtml(delivery.status)} · ${escapeHtml(delivery.channel)} · ${escapeHtml(delivery.webhook_configured ? "webhook configured" : "webhook not configured")} · ${formatInteger(delivery.target_count)} targets</div>`
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
