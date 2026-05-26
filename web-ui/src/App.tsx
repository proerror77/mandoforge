import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  appendUserMessage,
  api,
  createManagerAgentPlan,
  createSession,
  decideApproval,
  getAdminToken,
  reviewMemoryWritebackCandidate,
  reviewManagerAgentPlan,
  setAdminToken,
  transitionAgentHandoff,
  updateWorkflowDefinition,
  workflowPackAction,
  type AgentHandoffAssignment,
  type AgentHandoffEvent,
  type AgentInboxSnapshot,
  type Agent,
  type Approval,
  type Artifact,
  type ContextPacket,
  type DeploymentVersion,
  type Environment,
  type ManagerAgentPlan,
  type MemoryGovernancePartitionDetail,
  type MemoryGovernanceSummary,
  type MemoryGovernanceWritebackQueue,
  type MemoryWritebackCandidate,
  type OntologyRegistry,
  type ObservabilitySummary,
  type RunWorkflowStepRunResponse,
  type SchedulerOrchestrationSummary,
  type SemanticIngestionBatchResult,
  type SemanticLink,
  type SemanticObject,
  type SemanticRetrievalBackendRegistry,
  type SemanticSynthesisRunResult,
  type Session,
  type SessionEvent,
  type Stage2Readiness,
  type TaskGrant,
  type TaskBoardSnapshot,
  type ToolCall,
  type WorkerJob,
  type WorkflowGraphConsole,
  type WorkflowDefinition,
  type WorkflowPackBinding,
  type WorkflowPackConnectorQualityAssessment,
  type WorkflowPackInstallation,
  type WorkflowPackOnboardingAssessment,
  type WorkflowPackProfileAsset,
  type WorkflowPackRuntimeObject,
  type WorkflowRun,
  type WorkflowStepRun,
  type WorkflowTransition,
  type WorkItem,
} from "./api";

const DEFAULT_TASK =
  "检查 runtime 当前状态，列出 worker、tool call、approval 和 artifact 的真实运行证据。";

const DEFAULT_INGESTION_BATCH = JSON.stringify({
  source: {
    source_type: "repo_doc",
    source_uri: "repo://docs/semantic-ingestion-console.md",
    display_name: "Semantic ingestion console note",
    metadata: { path: "docs/semantic-ingestion-console.md" },
    provenance: { ingested_by: "web-ui" },
    freshness: { state: "operator_submitted" },
  },
  objects: [
    {
      temp_ref: "note",
      object_type: "memory",
      object_key: "memory:semantic-console:note",
      title: "Semantic console note",
      summary: "Operator submitted semantic memory through the ingestion batch endpoint.",
      content: { note: "Replace this sample before submitting." },
      semantic_scopes: {
        project_scope: "mandoforge",
        workflow_scope: "semantic-ingestion",
        memory_scope: "engineering",
        share_policy: "isolated",
      },
      provenance: { source: "console" },
      trust_level: "source_attested",
      freshness: "current",
    },
  ],
  links: [],
}, null, 2);

const DEFAULT_SYNTHESIS_RUN = JSON.stringify({
  synthesis_type: "post_run_reflection",
  goal_attempted: "Summarize the completed run into reviewable memory candidates.",
  context_used: ["session_events", "artifacts", "context_packets"],
  worked: ["Replace with evidence-backed observations."],
  failed_or_corrected: [],
  unsafe_assumptions: ["Do not promote synthesis output without review."],
  durable_memory_candidates: [
    {
      proposed_object_key: "memory:semantic-synthesis:replace-me",
      title: "Reviewable synthesis memory",
      summary: "Replace this sample with one evidence-backed durable memory candidate.",
      content: { note: "candidate-first" },
      trust_level: "source_attested",
      freshness: "current",
    },
  ],
}, null, 2);

const DEFAULT_PACK_PROFILE_ASSETS = JSON.stringify({
  profiles: [
    {
      id: "company",
      content: "# Company profile\n\nReplace this with tenant-specific company context before saving.",
    },
  ],
  reason: "operator configured onboarding profile assets from the pack console",
}, null, 2);

type RowStatus = "needs_input" | "running" | "queued" | "completed" | "failed" | "idle";

type SessionRow = {
  session: Session;
  agent?: Agent;
  workflowRun?: WorkflowRun;
  status: RowStatus;
  label: string;
  detail: string;
  latestJob?: WorkerJob;
  toolCount: number;
  approvalCount: number;
  updatedAt: string;
};

type SemanticLinkDraft = {
  from: string;
  relation: string;
  to: string;
};

type TaskBoardItemView = TaskBoardSnapshot["items"][number];
type TaskBoardColumnId = "ready" | "running" | "review" | "blocked" | "backlog" | "done";
type WorkbenchView = "agents" | "board" | "manager" | "semantic" | "packs" | "deploy";

const TASK_BOARD_COLUMNS: Array<{
  id: TaskBoardColumnId;
  title: string;
  hint: string;
}> = [
  { id: "ready", title: "Ready", hint: "Claimable work" },
  { id: "running", title: "Running", hint: "Leased or active" },
  { id: "review", title: "Review", hint: "Approval / action needed" },
  { id: "blocked", title: "Blocked", hint: "Dependency or grant missing" },
  { id: "backlog", title: "Backlog", hint: "Waiting for scheduler" },
  { id: "done", title: "Done", hint: "Terminal states" },
];

export function App() {
  const queryClient = useQueryClient();
  const [selectedSessionId, setSelectedSessionId] = useState(() => localStorage.getItem("mandoforge.activeSessionId") ?? "");
  const [selectedAgentId, setSelectedAgentId] = useState("");
  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState("");
  const [task, setTask] = useState(DEFAULT_TASK);
  const [adminTokenInput, setAdminTokenInput] = useState(() => consumeTokenFromHash() || getAdminToken());
  const [activeView, setActiveView] = useState<WorkbenchView>(() => {
    const value = localStorage.getItem("mandoforge.activeView");
    return isWorkbenchView(value) ? value : "agents";
  });
  const [transitionFilter, setTransitionFilter] = useState("all");
  const [transitionLimit, setTransitionLimit] = useState(50);
  const [memoryPartitionKey, setMemoryPartitionKey] = useState("");
  const [writebackStatus, setWritebackStatus] = useState("pending");
  const [writebackLimit, setWritebackLimit] = useState(50);
  const [selectedQueueWritebackId, setSelectedQueueWritebackId] = useState("");
  const [writebackReviewReason, setWritebackReviewReason] = useState("");
  const [selectedContextPacketId, setSelectedContextPacketId] = useState("");
  const [semanticObjectType, setSemanticObjectType] = useState("all");
  const [semanticIngestionDraft, setSemanticIngestionDraft] = useState(DEFAULT_INGESTION_BATCH);
  const [semanticSynthesisDraft, setSemanticSynthesisDraft] = useState(DEFAULT_SYNTHESIS_RUN);
  const [graphEditorDraft, setGraphEditorDraft] = useState("");
  const [graphEditorStatus, setGraphEditorStatus] = useState("");
  const [managerGoal, setManagerGoal] = useState("Review this work item, decompose it, route it to a specialist, and preserve review evidence.");
  const [managerRisk, setManagerRisk] = useState("medium");
  const [managerWorkItemId, setManagerWorkItemId] = useState("");
  const [managerSpecialistAgentId, setManagerSpecialistAgentId] = useState("");
  const [managerSteps, setManagerSteps] = useState("intake\nanalyze\nspecialist handoff\nreview result");
  const [packManifestPath, setPackManifestPath] = useState("packs/ai-governance/package.yaml");
  const [selectedPackInstallationId, setSelectedPackInstallationId] = useState("");
  const [packProfileDraft, setPackProfileDraft] = useState(DEFAULT_PACK_PROFILE_ASSETS);
  const [packReleaseEvidence, setPackReleaseEvidence] = useState(JSON.stringify({
    source: "pack-console",
    eval_gate: "operator-confirmed",
    release_gate: "operator-confirmed",
  }, null, 2));
  const [linkDraft, setLinkDraft] = useState({
    from: "",
    relation: "supports",
    to: "",
  });

  const agents = useQuery({ queryKey: ["agents"], queryFn: () => api<Agent[]>("/api/agents"), refetchInterval: 5000 });
  const environments = useQuery({ queryKey: ["environments"], queryFn: () => api<Environment[]>("/api/environments"), refetchInterval: 5000 });
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: () => api<Session[]>("/api/sessions"), refetchInterval: 1800 });
  const approvals = useQuery({ queryKey: ["approvals"], queryFn: () => api<Approval[]>("/api/approvals"), refetchInterval: 1800 });
  const executionJobs = useQuery({ queryKey: ["execution-jobs"], queryFn: () => api<WorkerJob[]>("/api/execution-jobs"), refetchInterval: 1500 });
  const sessionLoopJobs = useQuery({ queryKey: ["session-loop-jobs"], queryFn: () => api<WorkerJob[]>("/api/session-loop-jobs"), refetchInterval: 1500 });
  const allToolCalls = useQuery({ queryKey: ["tool-calls"], queryFn: () => api<ToolCall[]>("/api/tool-calls"), refetchInterval: 1800 });
  const workflowRuns = useQuery({ queryKey: ["workflow-runs"], queryFn: () => api<WorkflowRun[]>("/api/workflow-runs"), refetchInterval: 1600 });
  const workflowDefinitions = useQuery({ queryKey: ["workflow-definitions"], queryFn: () => api<WorkflowDefinition[]>("/api/workflow-definitions"), refetchInterval: 3000 });
  const taskBoard = useQuery({ queryKey: ["task-board"], queryFn: () => api<TaskBoardSnapshot>("/api/task-board"), refetchInterval: 1500 });
  const workItems = useQuery({ queryKey: ["work-items"], queryFn: () => api<WorkItem[]>("/api/work-items"), refetchInterval: 3000 });
  const managerPlans = useQuery({ queryKey: ["manager-plans"], queryFn: () => api<ManagerAgentPlan[]>("/api/manager-plans"), refetchInterval: 3000 });
  const agentHandoffs = useQuery({ queryKey: ["agent-handoffs"], queryFn: () => api<AgentHandoffEvent[]>("/api/agent-handoffs"), refetchInterval: 3000 });
  const agentHandoffAssignments = useQuery({
    queryKey: ["agent-handoff-assignments"],
    queryFn: () => api<AgentHandoffAssignment[]>("/api/agent-handoff-assignments"),
    refetchInterval: 3000,
  });
  const workflowPackInstallations = useQuery({
    queryKey: ["workflow-pack-installations"],
    queryFn: () => api<WorkflowPackInstallation[]>("/api/workflow-packs/installations"),
    refetchInterval: 5000,
  });
  const stage2Readiness = useQuery({
    queryKey: ["stage2-readiness"],
    queryFn: () => api<Stage2Readiness>("/api/stage2/readiness"),
    refetchInterval: 10000,
  });
  const codexDeploymentReadiness = useQuery({
    queryKey: ["codex-app-server-deployment"],
    queryFn: () => api<Record<string, unknown>>("/api/codex-app-server/deployment/validate", { method: "POST" }),
    refetchInterval: 15000,
  });
  const observabilitySummary = useQuery({
    queryKey: ["observability-summary"],
    queryFn: () => api<ObservabilitySummary>("/api/observability"),
    refetchInterval: 10000,
  });

  const rows = useMemo(() => buildRows({
    sessions: sessions.data ?? [],
    agents: agents.data ?? [],
    approvals: approvals.data ?? [],
    jobs: [...(sessionLoopJobs.data ?? []), ...(executionJobs.data ?? [])],
    toolCalls: allToolCalls.data ?? [],
    workflowRuns: workflowRuns.data ?? [],
  }), [agents.data, allToolCalls.data, approvals.data, executionJobs.data, sessionLoopJobs.data, sessions.data, workflowRuns.data]);

  const selectedSession = rows.find((row) => row.session.id === selectedSessionId)?.session ?? rows[0]?.session;
  const sessionId = selectedSession?.id ?? "";
  const selectedRow = rows.find((row) => row.session.id === sessionId);
  const selectedWorkflowRun = selectedRow?.workflowRun;
  const workflowRunId = selectedWorkflowRun?.id ?? "";
  const selectedWorkflowDefinition = workflowDefinitions.data?.find((definition) => definition.id === selectedWorkflowRun?.workflow_definition_id);
  const packInstallationId = selectedWorkflowRun?.pack_installation_id ?? "";
  const selectedPackInstallation = workflowPackInstallations.data?.find((installation) => installation.id === selectedPackInstallationId)
    ?? workflowPackInstallations.data?.[0];
  const packConsoleInstallationId = selectedPackInstallation?.id ?? "";
  const selectedAgent = agents.data?.find((agent) => agent.id === selectedSession?.agent_id) ?? preferredAgent(agents.data ?? []);
  const operatorAgent = agents.data?.find((agent) => agent.id === (selectedAgentId || selectedAgent?.id)) ?? selectedAgent;
  const selectedEnvironment = environments.data?.find((environment) => environment.id === selectedSession?.environment_id) ?? environments.data?.[0];
  const specialistAgents = useMemo(() => (agents.data ?? []).filter((agent) => agent.agent_role === "specialist"), [agents.data]);
  const managerSessions = useMemo(() => rows.filter((row) => row.agent?.agent_role === "manager"), [rows]);
  const inboxAgentId = operatorAgent?.id ?? "";
  const agentInbox = useQuery({
    queryKey: ["agent-inbox", inboxAgentId],
    queryFn: () => api<AgentInboxSnapshot>(`/api/agents/${inboxAgentId}/inbox`),
    enabled: Boolean(inboxAgentId),
    refetchInterval: 1500,
  });

  const events = useQuery({
    queryKey: ["session-events", sessionId],
    queryFn: () => api<SessionEvent[]>(`/api/sessions/${sessionId}/events`),
    enabled: Boolean(sessionId),
    refetchInterval: 1000,
  });
  const sessionToolCalls = useQuery({
    queryKey: ["session-tool-calls", sessionId],
    queryFn: () => api<ToolCall[]>(`/api/sessions/${sessionId}/tool-calls`),
    enabled: Boolean(sessionId),
    refetchInterval: 1200,
  });
  const artifacts = useQuery({
    queryKey: ["artifacts", sessionId],
    queryFn: () => api<Artifact[]>(`/api/sessions/${sessionId}/artifacts`),
    enabled: Boolean(sessionId),
    refetchInterval: 1200,
  });
  const workflowSteps = useQuery({
    queryKey: ["workflow-steps", workflowRunId],
    queryFn: () => api<WorkflowStepRun[]>(`/api/workflow-runs/${workflowRunId}/steps`),
    enabled: Boolean(workflowRunId),
    refetchInterval: 1200,
  });
  const transitionQuery = useMemo(() => {
    const params = new URLSearchParams();
    params.set("limit", String(transitionLimit));
    if (transitionFilter !== "all") params.set("transition_type", transitionFilter);
    return params.toString();
  }, [transitionFilter, transitionLimit]);
  const workflowTransitions = useQuery({
    queryKey: ["workflow-transitions", workflowRunId, transitionQuery],
    queryFn: () => api<WorkflowTransition[]>(`/api/workflow-runs/${workflowRunId}/transitions?${transitionQuery}`),
    enabled: Boolean(workflowRunId),
    refetchInterval: 1200,
  });
  const workflowGraph = useQuery({
    queryKey: ["workflow-graph", workflowRunId],
    queryFn: () => api<WorkflowGraphConsole>(`/api/workflow-runs/${workflowRunId}/graph`),
    enabled: Boolean(workflowRunId),
    refetchInterval: 1200,
  });
  const workflowTaskGrants = useQuery({
    queryKey: ["workflow-task-grants", workflowRunId],
    queryFn: () => api<TaskGrant[]>(`/api/workflow-runs/${workflowRunId}/task-grants`),
    enabled: Boolean(workflowRunId),
    refetchInterval: 1500,
  });
  useEffect(() => {
    if (!selectedWorkflowDefinition) {
      setGraphEditorDraft("");
      setGraphEditorStatus("");
      return;
    }
    setGraphEditorDraft(JSON.stringify(selectedWorkflowDefinition.step_graph, null, 2));
    setGraphEditorStatus("");
  }, [selectedWorkflowDefinition?.id, selectedWorkflowDefinition?.updated_at]);
  useEffect(() => {
    if (!managerWorkItemId && workItems.data?.[0]) {
      setManagerWorkItemId(workItems.data[0].id);
    }
  }, [managerWorkItemId, workItems.data]);
  useEffect(() => {
    if (!managerSpecialistAgentId && specialistAgents[0]) {
      setManagerSpecialistAgentId(specialistAgents[0].id);
    }
  }, [managerSpecialistAgentId, specialistAgents]);
  useEffect(() => {
    if (!selectedPackInstallationId && workflowPackInstallations.data?.[0]) {
      setSelectedPackInstallationId(workflowPackInstallations.data[0].id);
    }
  }, [selectedPackInstallationId, workflowPackInstallations.data]);
  const workflowPackBindings = useQuery({
    queryKey: ["workflow-pack-bindings", packInstallationId],
    queryFn: () => api<WorkflowPackBinding[]>(`/api/workflow-packs/installations/${packInstallationId}/bindings`),
    enabled: Boolean(packInstallationId),
    refetchInterval: 3000,
  });
  const workflowPackRuntimeObjects = useQuery({
    queryKey: ["workflow-pack-runtime-objects", packInstallationId],
    queryFn: () => api<WorkflowPackRuntimeObject[]>(`/api/workflow-packs/installations/${packInstallationId}/runtime-objects`),
    enabled: Boolean(packInstallationId),
    refetchInterval: 3000,
  });
  const packConsoleBindings = useQuery({
    queryKey: ["pack-console-bindings", packConsoleInstallationId],
    queryFn: () => api<WorkflowPackBinding[]>(`/api/workflow-packs/installations/${packConsoleInstallationId}/bindings`),
    enabled: Boolean(packConsoleInstallationId),
    refetchInterval: 3000,
  });
  const packConsoleRuntimeObjects = useQuery({
    queryKey: ["pack-console-runtime-objects", packConsoleInstallationId],
    queryFn: () => api<WorkflowPackRuntimeObject[]>(`/api/workflow-packs/installations/${packConsoleInstallationId}/runtime-objects`),
    enabled: Boolean(packConsoleInstallationId),
    refetchInterval: 3000,
  });
  const packProfileAssets = useQuery({
    queryKey: ["pack-profile-assets", packConsoleInstallationId],
    queryFn: () => api<WorkflowPackProfileAsset[]>(`/api/workflow-packs/installations/${packConsoleInstallationId}/onboarding/profiles`),
    enabled: Boolean(packConsoleInstallationId),
    refetchInterval: 5000,
  });
  const memoryGovernance = useQuery({
    queryKey: ["memory-governance-summary"],
    queryFn: () => api<MemoryGovernanceSummary>("/api/memory-governance/summary"),
    refetchInterval: 5000,
  });
  const selectedMemoryPartitionKey = memoryPartitionKey || memoryGovernance.data?.partitions[0]?.partition_key || "";
  const memoryPartition = useQuery({
    queryKey: ["memory-governance-partition", selectedMemoryPartitionKey],
    queryFn: () => api<MemoryGovernancePartitionDetail>(`/api/memory-governance/partitions?partition_key=${encodeURIComponent(selectedMemoryPartitionKey)}`),
    enabled: Boolean(selectedMemoryPartitionKey),
    refetchInterval: 5000,
  });
  const writebackQuery = useMemo(() => {
    const params = new URLSearchParams();
    params.set("limit", String(writebackLimit));
    if (writebackStatus !== "all") params.set("status", writebackStatus);
    return params.toString();
  }, [writebackLimit, writebackStatus]);
  const memoryWritebacks = useQuery({
    queryKey: ["memory-governance-writebacks", writebackQuery],
    queryFn: () => api<MemoryGovernanceWritebackQueue>(`/api/memory-governance/writebacks?${writebackQuery}`),
    refetchInterval: 5000,
  });
  const allWritebackCandidates = useQuery({
    queryKey: ["memory-writeback-candidates"],
    queryFn: () => api<MemoryWritebackCandidate[]>("/api/memory-writeback-candidates"),
    refetchInterval: 5000,
  });
  const schedulerSummary = useQuery({
    queryKey: ["scheduler-summary"],
    queryFn: () => api<SchedulerOrchestrationSummary>("/api/scheduler/summary"),
    refetchInterval: 5000,
  });
  const deploymentVersion = useQuery({
    queryKey: ["deployment-version"],
    queryFn: () => api<DeploymentVersion>("/api/deployment/version"),
    refetchInterval: 30000,
  });
  const semanticObjects = useQuery({
    queryKey: ["semantic-objects"],
    queryFn: () => api<SemanticObject[]>("/api/semantic-objects"),
    refetchInterval: 5000,
  });
  const semanticLinks = useQuery({
    queryKey: ["semantic-links"],
    queryFn: () => api<SemanticLink[]>("/api/semantic-links"),
    refetchInterval: 5000,
  });
  const ontologyRegistry = useQuery({
    queryKey: ["ontology-registry"],
    queryFn: () => api<OntologyRegistry>("/api/ontology/registry"),
    refetchInterval: 30000,
  });
  const semanticBackends = useQuery({
    queryKey: ["semantic-retrieval-backends"],
    queryFn: () => api<SemanticRetrievalBackendRegistry>("/api/semantic-retrieval/backends"),
    refetchInterval: 10000,
  });
  const contextPackets = useQuery({
    queryKey: ["context-packets", sessionId],
    queryFn: () => api<ContextPacket[]>(`/api/sessions/${sessionId}/context-packets`),
    enabled: Boolean(sessionId),
    refetchInterval: 3000,
  });
  const sessionWritebackCandidates = useQuery({
    queryKey: ["session-writeback-candidates", sessionId],
    queryFn: () => api<MemoryWritebackCandidate[]>(`/api/sessions/${sessionId}/memory-writeback-candidates`),
    enabled: Boolean(sessionId),
    refetchInterval: 3000,
  });
  const visibleSemanticObjects = useMemo(() => {
    const objects = semanticObjects.data ?? [];
    return semanticObjectType === "all"
      ? objects
      : objects.filter((object) => object.object_type === semanticObjectType);
  }, [semanticObjectType, semanticObjects.data]);
  const semanticObjectTypes = useMemo(() => {
    return ["all", ...Array.from(new Set((semanticObjects.data ?? []).map((object) => object.object_type))).sort()];
  }, [semanticObjects.data]);
  const contextPacket = useMemo(() => {
    const packets = contextPackets.data ?? [];
    return packets.find((packet) => packet.id === selectedContextPacketId) ?? packets[0];
  }, [contextPackets.data, selectedContextPacketId]);
  const visibleToolCalls = sessionToolCalls.data?.length
    ? sessionToolCalls.data
    : (allToolCalls.data ?? []).filter((call) => call.session_id === sessionId);
  const runtime = runtimeSummary(events.data ?? [], selectedAgent);
  const transitionTypes = uniqueTransitionTypes(workflowGraph.data?.edges ?? workflowTransitions.data ?? []);
  const filteredTransitions = workflowTransitions.data ?? [];

  const launch = useMutation({
    mutationFn: async () => {
      const agent = agents.data?.find((candidate) => candidate.id === selectedAgentId) ?? preferredAgent(agents.data ?? []);
      if (!agent) throw new Error("No agent available");
      const environment = environments.data?.find((candidate) => candidate.id === selectedEnvironmentId) ?? environments.data?.[0];
      const session = await createSession({
        agent_id: agent.id,
        environment_id: environment?.id,
        title: titleFromTask(task),
      });
      localStorage.setItem("mandoforge.activeSessionId", session.id);
      setSelectedSessionId(session.id);
      await appendUserMessage(session.id, `${task.trim()}\n\nExpose progress through events, tool calls, worker jobs, approvals, and artifacts.`);
      return session;
    },
    onSuccess: () => invalidateAll(queryClient),
  });

  const decide = useMutation({
    mutationFn: ({ id, decision }: { id: string; decision: "approve" | "reject" }) => decideApproval(id, decision),
    onSuccess: () => invalidateAll(queryClient),
  });
  const runStep = useMutation({
    mutationFn: ({ stepId, agentId }: { stepId: string; agentId: string }) => api<RunWorkflowStepRunResponse>(`/api/workflow-step-runs/${stepId}/run`, {
      method: "POST",
      body: JSON.stringify({
        agent_id: agentId,
        worker_id: "ui-operator",
        lease_seconds: 600,
      }),
    }),
    onSuccess: () => invalidateAll(queryClient),
  });
  const createContextPacket = useMutation({
    mutationFn: () => api<ContextPacket>(`/api/sessions/${sessionId}/context-packet`, {
      method: "POST",
      body: JSON.stringify({}),
    }),
    onSuccess: (packet) => {
      setSelectedContextPacketId(packet.id);
      invalidateAll(queryClient);
    },
  });
  const generateWritebacks = useMutation({
    mutationFn: () => api<MemoryWritebackCandidate[]>(`/api/sessions/${sessionId}/memory-writeback-candidates`, {
      method: "POST",
      body: JSON.stringify({
        include_session_summary: true,
        include_artifacts: true,
        include_handoffs: true,
        include_approvals: true,
      }),
    }),
    onSuccess: () => invalidateAll(queryClient),
  });
  const reviewWriteback = useMutation({
    mutationFn: ({ id, decision, reason }: { id: string; decision: "approve" | "reject"; reason?: string }) => (
      reviewMemoryWritebackCandidate(id, decision, reason)
    ),
    onSuccess: () => {
      setWritebackReviewReason("");
      invalidateAll(queryClient);
    },
  });
  const updateWorkflowGraph = useMutation({
    mutationFn: () => {
      if (!selectedWorkflowDefinition) throw new Error("No workflow definition selected");
      const parsed = JSON.parse(graphEditorDraft) as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error("step_graph must be a JSON object");
      }
      return updateWorkflowDefinition(selectedWorkflowDefinition.id, { step_graph: parsed as Record<string, unknown> });
    },
    onSuccess: (definition) => {
      setGraphEditorStatus(`Saved ${definition.updated_at}`);
      invalidateAll(queryClient);
    },
  });
  const createSemanticLink = useMutation({
    mutationFn: () => {
      if (!linkDraft.from || !linkDraft.to || !linkDraft.relation.trim()) {
        throw new Error("semantic link requires two objects and a relation");
      }
      return api<SemanticLink>("/api/semantic-links", {
        method: "POST",
        body: JSON.stringify({
          from_entity_type: "semantic_object",
          from_entity_id: linkDraft.from,
          relation_type: linkDraft.relation.trim(),
          to_entity_type: "semantic_object",
          to_entity_id: linkDraft.to,
          confidence: 0.8,
          metadata: { source: "semantic-console" },
          provenance: { created_from: "web-ui" },
          status: "active",
        }),
      });
    },
    onSuccess: () => {
      setLinkDraft((draft) => ({ ...draft, relation: "supports" }));
      invalidateAll(queryClient);
    },
  });
  const createSemanticIngestionBatch = useMutation({
    mutationFn: () => {
      let payload: unknown;
      try {
        payload = JSON.parse(semanticIngestionDraft);
      } catch (error) {
        throw new Error(`invalid ingestion JSON: ${errorMessage(error)}`);
      }
      return api<SemanticIngestionBatchResult>("/api/semantic-ingestion/batches", {
        method: "POST",
        body: JSON.stringify(payload),
      });
    },
    onSuccess: () => invalidateAll(queryClient),
  });
  const createSemanticSynthesisRun = useMutation({
    mutationFn: () => {
      if (!sessionId) {
        throw new Error("select a session before creating semantic synthesis");
      }
      let payload: unknown;
      try {
        payload = JSON.parse(semanticSynthesisDraft);
      } catch (error) {
        throw new Error(`invalid synthesis JSON: ${errorMessage(error)}`);
      }
      return api<SemanticSynthesisRunResult>(`/api/sessions/${sessionId}/semantic-synthesis-runs`, {
        method: "POST",
        body: JSON.stringify(payload),
      });
    },
    onSuccess: () => invalidateAll(queryClient),
  });
  const createManagerPlanMutation = useMutation({
    mutationFn: () => {
      const managerSession = selectedSession?.agent_id && selectedAgent?.agent_role === "manager"
        ? selectedSession
        : managerSessions[0]?.session;
      if (!managerSession) {
        throw new Error("create or select a manager-agent session before creating a manager plan");
      }
      const steps = managerSteps
        .split("\n")
        .map((step) => step.trim())
        .filter(Boolean);
      return createManagerAgentPlan(managerSession.id, {
        work_item_id: managerWorkItemId || null,
        specialist_agent_id: managerSpecialistAgentId || null,
        task_intake: {
          goal: managerGoal.trim(),
          source: "manager-console",
          work_item_id: managerWorkItemId || null,
        },
        decomposition: {
          strategy: "operator_seeded",
          steps,
        },
        specialist_selection: {
          specialist_agent_id: managerSpecialistAgentId || null,
          reason: "selected from manager console",
        },
        risk_classification: managerRisk,
        review: {},
      });
    },
    onSuccess: () => invalidateAll(queryClient),
  });
  const reviewManagerPlanMutation = useMutation({
    mutationFn: ({ id, status }: { id: string; status: string }) => reviewManagerAgentPlan(id, {
      status,
      review: {
        reviewed_by: "web-ui",
        decision: status,
        reviewed_at: new Date().toISOString(),
      },
    }),
    onSuccess: () => invalidateAll(queryClient),
  });
  const packValidate = useMutation({
    mutationFn: () => workflowPackAction<Record<string, unknown>>("/api/workflow-packs/validate", { manifest_path: packManifestPath }),
  });
  const packInstall = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackInstallation>("/api/workflow-packs/install", { manifest_path: packManifestPath }),
    onSuccess: (installation) => {
      setSelectedPackInstallationId(installation.id);
      invalidateAll(queryClient);
    },
  });
  const packOnboardingAssess = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackOnboardingAssessment>(
      `/api/workflow-packs/installations/${packConsoleInstallationId}/onboarding/assess`,
      { reason: "operator assessed onboarding readiness from the pack console" },
    ),
    onSuccess: () => invalidateAll(queryClient),
  });
  const packProfilesSave = useMutation({
    mutationFn: () => {
      let payload: Record<string, unknown>;
      try {
        payload = JSON.parse(packProfileDraft) as Record<string, unknown>;
      } catch (error) {
        throw new Error(`invalid profile JSON: ${errorMessage(error)}`);
      }
      return workflowPackAction<WorkflowPackProfileAsset[]>(
        `/api/workflow-packs/installations/${packConsoleInstallationId}/onboarding/profiles`,
        payload,
      );
    },
    onSuccess: () => invalidateAll(queryClient),
  });
  const packConnectorQualityAssess = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackConnectorQualityAssessment>(
      `/api/workflow-packs/installations/${packConsoleInstallationId}/connectors/quality/assess`,
      {
        connectors: [],
        reason: "operator assessed connector quality from the pack console",
      },
    ),
    onSuccess: () => invalidateAll(queryClient),
  });
  const packUpdate = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackInstallation>(
      `/api/workflow-packs/installations/${packConsoleInstallationId}/update`,
      {
        manifest_path: packManifestPath,
        reason: "operator created a new workflow pack version from the pack console",
      },
    ),
    onSuccess: (installation) => {
      setSelectedPackInstallationId(installation.id);
      invalidateAll(queryClient);
    },
  });
  const packArchive = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackInstallation>(
      `/api/workflow-packs/installations/${packConsoleInstallationId}/archive`,
      { reason: "operator archived workflow pack installation from the pack console" },
    ),
    onSuccess: () => {
      setSelectedPackInstallationId("");
      invalidateAll(queryClient);
    },
  });
  const handoffTransition = useMutation({
    mutationFn: ({ id, action }: { id: string; action: "accept" | "reject" | "fail" | "complete" | "escalate" }) => transitionAgentHandoff(
      id,
      action,
      action === "escalate"
        ? { reason: "operator escalated from manager desk", status: "requested" }
        : { reason: `operator ${action} from manager desk` },
    ),
    onSuccess: () => invalidateAll(queryClient),
  });
  const packStage = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackInstallation>(
      `/api/workflow-packs/installations/${packConsoleInstallationId}/stage`,
      { reason: "operator staged pack from the pack console" },
    ),
    onSuccess: () => invalidateAll(queryClient),
  });
  const packRelease = useMutation({
    mutationFn: () => {
      let evidence: Record<string, unknown>;
      try {
        evidence = JSON.parse(packReleaseEvidence) as Record<string, unknown>;
      } catch (error) {
        throw new Error(`invalid release evidence JSON: ${errorMessage(error)}`);
      }
      return workflowPackAction<WorkflowPackInstallation>(
        `/api/workflow-packs/installations/${packConsoleInstallationId}/release`,
        {
          eval_gate_status: "passed",
          release_gate_status: "passed",
          gate_evidence: evidence,
          reason: "operator released pack from the pack console",
        },
      );
    },
    onSuccess: () => invalidateAll(queryClient),
  });
  const packRollback = useMutation({
    mutationFn: () => workflowPackAction<WorkflowPackInstallation>(
      `/api/workflow-packs/installations/${packConsoleInstallationId}/rollback`,
      {
        gate_evidence: { source: "pack-console", requested_at: new Date().toISOString() },
        reason: "operator rolled back pack from the pack console",
      },
    ),
    onSuccess: () => invalidateAll(queryClient),
  });
  const schedulerRunDue = useMutation({
    mutationFn: () => api<Record<string, unknown>>("/api/scheduler/run-due", {
      method: "POST",
      body: JSON.stringify({ owner: "web-ui", idempotency_key: `ui-${Date.now()}` }),
    }),
    onSuccess: () => invalidateAll(queryClient),
  });

  const activeCount = rows.filter((row) => row.status === "running" || row.status === "queued").length;
  const blockedCount = rows.filter((row) => row.status === "needs_input").length;
  const apiError = firstQueryError([
    agents.error,
    environments.error,
    sessions.error,
    approvals.error,
    executionJobs.error,
    sessionLoopJobs.error,
    allToolCalls.error,
    taskBoard.error,
    workItems.error,
    managerPlans.error,
    agentHandoffs.error,
    agentHandoffAssignments.error,
    agentInbox.error,
    workflowTransitions.error,
    workflowDefinitions.error,
    workflowPackInstallations.error,
    workflowPackBindings.error,
    workflowPackRuntimeObjects.error,
    memoryGovernance.error,
    memoryWritebacks.error,
    allWritebackCandidates.error,
    semanticObjects.error,
    semanticLinks.error,
    ontologyRegistry.error,
    createSemanticIngestionBatch.error,
    semanticBackends.error,
    contextPackets.error,
    sessionWritebackCandidates.error,
  ]);

  return (
    <main className="workbench">
      <header className="workbench-top">
        <div>
          <p className="eyebrow">MandoForge Co-Work</p>
          <h1>{viewTitle(activeView)}</h1>
        </div>
        <div className="top-metrics">
          <Metric label="Agents" value={String(agents.data?.length ?? 0)} />
          <Metric label="Running" value={String(activeCount)} tone={activeCount ? "live" : undefined} />
          <Metric label="Needs input" value={String(blockedCount)} tone={blockedCount ? "warn" : undefined} />
          <Metric label="Sessions" value={String(rows.length)} />
          <Metric label="Workflows" value={String(workflowRuns.data?.length ?? 0)} />
          <Metric label="Claimable" value={String(taskBoard.data?.claimable_count ?? 0)} tone={(taskBoard.data?.claimable_count ?? 0) ? "live" : undefined} />
        </div>
      </header>

      <section className={apiError ? "auth-strip auth-error" : "auth-strip"}>
        <div>
          <strong>{apiError ? "API access blocked" : "API connected"}</strong>
          <span>{apiError ?? "Authenticated requests are enabled for live agents, sessions, workers, approvals, and logs."}</span>
        </div>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            setAdminToken(adminTokenInput);
            invalidateAll(queryClient);
          }}
        >
          <input
            value={adminTokenInput}
            onChange={(event) => setAdminTokenInput(event.target.value)}
            placeholder="Dev admin token"
            type="password"
          />
          <button type="submit">Connect</button>
        </form>
      </section>

      <nav className="workspace-tabs" aria-label="Workspace views">
        {[
          { id: "agents" as const, label: "Agents", detail: "runtime, logs, approvals" },
          { id: "board" as const, label: "Board", detail: "work items and inbox" },
          { id: "manager" as const, label: "Manager", detail: "plans, routing, review" },
          { id: "semantic" as const, label: "Semantic", detail: "memory and ontology" },
          { id: "packs" as const, label: "Packs", detail: "install, configure, release" },
          { id: "deploy" as const, label: "Deploy", detail: "latest and readiness" },
        ].map((view) => (
          <button
            key={view.id}
            className={activeView === view.id ? "workspace-tab active" : "workspace-tab"}
            onClick={() => {
              setActiveView(view.id);
              localStorage.setItem("mandoforge.activeView", view.id);
            }}
          >
            <strong>{view.label}</strong>
            <span>{view.detail}</span>
          </button>
        ))}
      </nav>

      {activeView === "agents" ? (
      <section className="workbench-grid">
        <aside className="agent-lane">
          <div className="lane-title">
            <h2>Agents running</h2>
            <span>{rows.length} sessions</span>
          </div>
          <div className="session-stack">
            {rows.map((row) => (
              <button
                key={row.session.id}
                className={row.session.id === sessionId ? `session-row selected status-${row.status}` : `session-row status-${row.status}`}
                onClick={() => {
                  localStorage.setItem("mandoforge.activeSessionId", row.session.id);
                  setSelectedSessionId(row.session.id);
                }}
              >
                <StatusLogo status={row.status} />
                <span className="session-copy">
                  <strong>{row.session.title}</strong>
                  <small>{row.agent?.name ?? "Unassigned agent"} · {row.detail}</small>
                </span>
                <span className="session-time">{relativeAge(row.updatedAt)}</span>
              </button>
            ))}
          </div>
        </aside>

        <section className="log-console">
          <div className="console-head">
            <div>
              <p className="eyebrow">Live log stream</p>
              <h2>{selectedSession?.title ?? "No session selected"}</h2>
              <p>{selectedAgent?.name ?? "No agent"} · {selectedEnvironment?.name ?? "No environment"}</p>
            </div>
            <span className={`state-pill state-${selectedRow?.status ?? "idle"}`}>
              <StatusLogo status={selectedRow?.status ?? "idle"} />
              {selectedRow?.label ?? "Idle"}
            </span>
          </div>

          <div className="phase-strip">
            {["workflow", "session", "llm", "policy", "tool", "artifact"].map((phase) => (
              <span key={phase} className={hasPhase(events.data ?? [], workflowTransitions.data ?? [], phase) ? "phase-on" : ""}>{phase}</span>
            ))}
          </div>

          <div className="log-list">
            {(events.data ?? []).length ? [...(events.data ?? [])].slice(-80).reverse().map((event) => (
              <article key={`${event.seq}-${event.event_type}`} className={`log-line log-${eventKind(event.event_type)}`}>
                <span className="log-seq">#{event.seq}</span>
                <div>
                  <strong>{event.event_type}</strong>
                  <small>{event.created_at}</small>
                  <pre>{JSON.stringify(event.payload, null, 2)}</pre>
                </div>
              </article>
            )) : (
              <div className="empty-log">No events yet. Launch a task to start the worker loop.</div>
            )}
          </div>
        </section>

        <aside className="observer-panel">
          <Panel title="Workflow">
            {selectedWorkflowRun ? (
              <>
                <KeyValue label="Run status" value={selectedWorkflowRun.status} />
                <KeyValue label="Run ID" value={shortId(selectedWorkflowRun.id)} />
                <KeyValue label="Definition" value={shortId(selectedWorkflowRun.workflow_definition_id)} />
                <KeyValue label="Pack" value={shortId(selectedWorkflowRun.pack_installation_id ?? "none")} />
                <KeyValue label="Root grant" value={shortId(selectedWorkflowRun.root_task_grant_id ?? "none")} />
              </>
            ) : <p className="muted">No workflow run is linked to this session.</p>}
          </Panel>

          <Panel title="Graph console">
            {workflowGraph.data ? (
              <WorkflowGraph graph={workflowGraph.data} />
            ) : <p className="muted">No graph nodes observed.</p>}
          </Panel>

          <Panel title="Graph authoring">
            <WorkflowDefinitionEditor
              definition={selectedWorkflowDefinition}
              draft={graphEditorDraft}
              statusMessage={graphEditorStatus}
              isSaving={updateWorkflowGraph.isPending}
              error={updateWorkflowGraph.error}
              onDraftChange={(value) => {
                setGraphEditorDraft(value);
                setGraphEditorStatus("");
              }}
              onApply={() => updateWorkflowGraph.mutate()}
              onReset={() => {
                if (selectedWorkflowDefinition) {
                  setGraphEditorDraft(JSON.stringify(selectedWorkflowDefinition.step_graph, null, 2));
                  setGraphEditorStatus("");
                }
              }}
            />
          </Panel>

          <Panel title="Steps">
            {(workflowSteps.data ?? []).length ? (workflowSteps.data ?? []).map((step) => (
              <StepRow key={step.id} step={step} />
            )) : <p className="muted">No workflow steps observed.</p>}
          </Panel>

          <Panel title="Transitions">
            {selectedWorkflowRun ? (
              <div className="control-grid">
                <label>
                  <span>transition_type</span>
                  <select value={transitionFilter} onChange={(event) => setTransitionFilter(event.target.value)}>
                    {["all", ...transitionTypes].map((type) => (
                      <option key={type} value={type}>{type}</option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>limit</span>
                  <input
                    type="number"
                    min={1}
                    max={500}
                    value={transitionLimit}
                    onChange={(event) => setTransitionLimit(clampNumber(event.target.valueAsNumber, 1, 500, 50))}
                  />
                </label>
                <span className="status-hint">
                  {workflowTransitions.isFetching ? "Refreshing" : `${filteredTransitions.length} records`} · limit {transitionLimit}
                </span>
              </div>
            ) : null}
            {workflowTransitions.error ? <p className="error-note">{errorMessage(workflowTransitions.error)}</p> : null}
            {filteredTransitions.length ? filteredTransitions.slice(-10).reverse().map((transition) => (
              <TransitionRow key={transition.id} transition={transition} />
            )) : <p className="muted">No durable transition records yet.</p>}
          </Panel>

          <Panel title="Pack runtime">
            {packInstallationId ? (
              <PackRuntimePanel
                packInstallationId={packInstallationId}
                runtimeObjects={workflowPackRuntimeObjects.data ?? []}
                bindings={workflowPackBindings.data ?? []}
                isLoading={workflowPackRuntimeObjects.isLoading || workflowPackBindings.isLoading}
                error={workflowPackRuntimeObjects.error ?? workflowPackBindings.error}
              />
            ) : <p className="muted">This workflow is not bound to a pack installation.</p>}
          </Panel>

          <Panel title="Scheduler">
            {schedulerSummary.data ? (
              <SchedulerPanel summary={schedulerSummary.data} />
            ) : <p className="muted">Scheduler summary is loading.</p>}
          </Panel>

          <Panel title="Memory governance">
            {memoryGovernance.data ? (
              <>
                <KeyValue label="Status" value={memoryGovernance.data.status} />
                <KeyValue label="Isolation" value={memoryGovernance.data.isolation_policy} />
                <KeyValue label="Memory objects" value={String(memoryGovernance.data.memory_object_count)} />
                <KeyValue label="Partitions" value={String(memoryGovernance.data.partition_count)} />
                <div className="scope-metrics">
                  <MiniCountMap title="Trust" counts={memoryGovernance.data.trust_counts} />
                  <MiniCountMap title="Freshness" counts={memoryGovernance.data.freshness_counts} />
                  <div className="mini-counts">
                    <strong>Writeback</strong>
                    <span>{memoryGovernance.data.writeback.pending_count} pending</span>
                    <span>{memoryGovernance.data.writeback.approved_count} approved</span>
                    <span>{memoryGovernance.data.writeback.rejected_count} rejected</span>
                  </div>
                </div>
                <div className="partition-list">
                  {memoryGovernance.data.partitions.slice(0, 8).map((partition) => (
                    <button
                      key={partition.partition_key}
                      className={selectedMemoryPartitionKey === partition.partition_key ? "partition-chip selected" : "partition-chip"}
                      onClick={() => setMemoryPartitionKey(partition.partition_key)}
                    >
                      <span>{partition.domain_scope}/{partition.workflow_scope}</span>
                      <strong>{partition.memory_object_count}</strong>
                    </button>
                  ))}
                </div>
                {memoryPartition.data ? (
                  <MemoryPartitionDetail detail={memoryPartition.data} />
                ) : <p className="muted">No partition detail loaded.</p>}
                {memoryGovernance.data.attention_items.slice(0, 4).map((item) => (
                  <Row key={`${item.kind}-${item.message}`} title={`${item.severity} · ${item.kind}`} detail={item.message} />
                ))}
              </>
            ) : <p className="muted">Memory governance summary is not loaded.</p>}
          </Panel>

          <Panel title="Semantic retrieval">
            {semanticBackends.data ? (
              <SemanticBackendPanel registry={semanticBackends.data} />
            ) : <p className="muted">Semantic backend registry is loading.</p>}
          </Panel>

          <Panel title="Context packet trace">
            <div className="action-row">
              <button disabled={!sessionId || createContextPacket.isPending} onClick={() => createContextPacket.mutate()}>
                {createContextPacket.isPending ? "Building..." : "Build packet"}
              </button>
              <span>{contextPackets.data?.length ?? 0} packet versions</span>
            </div>
            {(contextPackets.data ?? []).length ? (
              <select value={contextPacket?.id ?? ""} onChange={(event) => setSelectedContextPacketId(event.target.value)}>
                {(contextPackets.data ?? []).map((packet) => (
                  <option key={packet.id} value={packet.id}>
                    v{packet.version} · {shortId(packet.id)} · {relativeAge(packet.generated_at)}
                  </option>
                ))}
              </select>
            ) : null}
            {createContextPacket.error ? <p className="error-note">{errorMessage(createContextPacket.error)}</p> : null}
            {contextPacket ? (
              <ContextPacketTrace packet={contextPacket} />
            ) : <p className="muted">No context packet has been generated for this session.</p>}
          </Panel>

          <Panel title="Semantic synthesis">
            <SemanticSynthesisPanel
              sessionId={sessionId}
              draft={semanticSynthesisDraft}
              result={createSemanticSynthesisRun.data}
              isSaving={createSemanticSynthesisRun.isPending}
              error={createSemanticSynthesisRun.error}
              onDraftChange={setSemanticSynthesisDraft}
              onCreate={() => createSemanticSynthesisRun.mutate()}
            />
          </Panel>

          <Panel title="Writeback review">
            <div className="action-row">
              <button disabled={!sessionId || generateWritebacks.isPending} onClick={() => generateWritebacks.mutate()}>
                {generateWritebacks.isPending ? "Generating..." : "Generate candidates"}
              </button>
              <span>{sessionWritebackCandidates.data?.filter((candidate) => candidate.status === "pending").length ?? 0} pending in session</span>
            </div>
            {generateWritebacks.error ? <p className="error-note">{errorMessage(generateWritebacks.error)}</p> : null}
            {reviewWriteback.error ? <p className="error-note">{errorMessage(reviewWriteback.error)}</p> : null}
            <WritebackReviewPanel
              candidates={sessionWritebackCandidates.data ?? []}
              queue={memoryWritebacks.data}
              isReviewing={reviewWriteback.isPending}
              onReview={(id, decision) => reviewWriteback.mutate({ id, decision })}
            />
          </Panel>

          <Panel title="Ontology links">
            <OntologyRegistryPanel registry={ontologyRegistry.data} />
            <SemanticIngestionPanel
              draft={semanticIngestionDraft}
              result={createSemanticIngestionBatch.data}
              isSaving={createSemanticIngestionBatch.isPending}
              error={createSemanticIngestionBatch.error}
              onDraftChange={setSemanticIngestionDraft}
              onCreate={() => createSemanticIngestionBatch.mutate()}
            />
            <SemanticObjectBrowser
              objects={visibleSemanticObjects}
              objectTypes={semanticObjectTypes}
              selectedType={semanticObjectType}
              onTypeChange={setSemanticObjectType}
            />
            <SemanticLinkManager
              objects={semanticObjects.data ?? []}
              links={semanticLinks.data ?? []}
              draft={linkDraft}
              registry={ontologyRegistry.data}
              isSaving={createSemanticLink.isPending}
              error={createSemanticLink.error}
              onDraftChange={setLinkDraft}
              onCreate={() => createSemanticLink.mutate()}
            />
          </Panel>

          <Panel title="Memory writeback queue">
            {memoryGovernance.data ? (
              <>
                {memoryWritebacks.data ? (
                  <MemoryWritebackQueuePanel
                    queue={memoryWritebacks.data}
                    fullCandidates={allWritebackCandidates.data ?? []}
                    status={writebackStatus}
                    limit={writebackLimit}
                    selectedId={selectedQueueWritebackId}
                    reviewReason={writebackReviewReason}
                    isLoadingDetail={allWritebackCandidates.isLoading}
                    isReviewing={reviewWriteback.isPending}
                    error={reviewWriteback.error ?? allWritebackCandidates.error}
                    onStatusChange={(status) => {
                      setWritebackStatus(status);
                      setSelectedQueueWritebackId("");
                    }}
                    onLimitChange={setWritebackLimit}
                    onSelect={setSelectedQueueWritebackId}
                    onReasonChange={setWritebackReviewReason}
                    onReview={(id, decision, reason) => reviewWriteback.mutate({ id, decision, reason })}
                  />
                ) : <p className="muted">Writeback queue not loaded.</p>}
              </>
            ) : <p className="muted">Memory governance summary is not loaded.</p>}
          </Panel>

          <Panel title="Worker">
            <KeyValue label="Latest job" value={selectedRow?.latestJob?.reason ?? selectedRow?.latestJob?.tool_name ?? "none"} />
            <KeyValue label="Worker" value={selectedRow?.latestJob?.worker_id ?? "waiting"} />
            <KeyValue label="Job status" value={selectedRow?.latestJob?.status ?? selectedRow?.status ?? "idle"} />
          </Panel>

          <Panel title="Runtime">
            <KeyValue label="Provider" value={runtime.provider} />
            <KeyValue label="Provider client" value={runtime.client} />
            <KeyValue label="Execution" value={runtime.execution} />
          </Panel>

          <Panel title="Approvals">
            {(approvals.data ?? []).filter((approval) => approval.session_id === sessionId && approval.status === "pending").length ? (
              (approvals.data ?? []).filter((approval) => approval.session_id === sessionId && approval.status === "pending").map((approval) => (
                <article key={approval.id} className="approval-card">
                  <strong>{approval.action}</strong>
                  <p>{approval.reason}</p>
                  <div>
                    <button disabled={decide.isPending} onClick={() => decide.mutate({ id: approval.id, decision: "approve" })}>Approve</button>
                    <button disabled={decide.isPending} className="ghost danger" onClick={() => decide.mutate({ id: approval.id, decision: "reject" })}>Reject</button>
                  </div>
                </article>
              ))
            ) : <p className="muted">No pending approval.</p>}
          </Panel>

          <Panel title="Grants">
            {(workflowTaskGrants.data ?? []).length ? (workflowTaskGrants.data ?? []).map((grant) => (
              <GrantRow key={grant.id} grant={grant} />
            )) : <p className="muted">No task grant for this session.</p>}
          </Panel>

          <Panel title="Tool calls">
            {visibleToolCalls.length ? visibleToolCalls.map((call) => (
              <Row key={call.id} title={call.tool_name} detail={`${call.status} · ${call.risk_level}`} />
            )) : <p className="muted">No tool call yet.</p>}
          </Panel>

          <Panel title="Artifacts">
            {(artifacts.data ?? []).length ? (artifacts.data ?? []).map((artifact) => (
              <Row key={artifact.id} title={artifact.name} detail={artifact.artifact_type} />
            )) : <p className="muted">No artifact yet.</p>}
          </Panel>
        </aside>
      </section>
      ) : activeView === "board" ? (
        <section className="board-workspace-page">
          <div className="board-page-main">
            <Panel title="Task board">
              {taskBoard.error ? (
                <p className="error-note">Task board failed: {errorMessage(taskBoard.error)}</p>
              ) : taskBoard.data ? (
                <TaskBoardPanel
                  board={taskBoard.data}
                  agents={agents.data ?? []}
                  selectedAgent={operatorAgent}
                  isRunning={runStep.isPending}
                  runError={runStep.error}
                  onRun={(stepId) => {
                    if (operatorAgent) runStep.mutate({ stepId, agentId: operatorAgent.id });
                  }}
                />
              ) : <p className="muted">Task board is loading.</p>}
            </Panel>
          </div>
          <aside className="board-page-side">
            <Panel title="Agent inbox">
              {agentInbox.error ? (
                <p className="error-note">Agent inbox failed: {errorMessage(agentInbox.error)}</p>
              ) : agentInbox.data ? (
                <AgentInboxPanel
                  inbox={agentInbox.data}
                  agent={operatorAgent}
                  isRunning={runStep.isPending}
                  onRun={(stepId) => {
                    if (operatorAgent) runStep.mutate({ stepId, agentId: operatorAgent.id });
                  }}
                />
              ) : <p className="muted">No agent inbox loaded.</p>}
            </Panel>

            <Panel title="Selected workflow">
              {selectedWorkflowRun ? (
                <>
                  <KeyValue label="Run status" value={selectedWorkflowRun.status} />
                  <KeyValue label="Run ID" value={shortId(selectedWorkflowRun.id)} />
                  <KeyValue label="Definition" value={shortId(selectedWorkflowRun.workflow_definition_id)} />
                  <KeyValue label="Root grant" value={shortId(selectedWorkflowRun.root_task_grant_id ?? "none")} />
                </>
              ) : <p className="muted">No workflow run is linked to this session.</p>}
            </Panel>

            <Panel title="Graph console">
              {workflowGraph.data ? (
                <WorkflowGraph graph={workflowGraph.data} />
              ) : <p className="muted">No graph nodes observed.</p>}
            </Panel>

            <Panel title="Runtime">
              <KeyValue label="Provider" value={runtime.provider} />
              <KeyValue label="Provider client" value={runtime.client} />
              <KeyValue label="Execution" value={runtime.execution} />
            </Panel>
          </aside>
        </section>
      ) : activeView === "manager" ? (
        <section className="product-page manager-page">
          <div className="product-main">
            <Panel title="Manager desk">
              <ManagerPlanComposer
                sessions={managerSessions}
                workItems={workItems.data ?? []}
                specialists={specialistAgents}
                goal={managerGoal}
                risk={managerRisk}
                workItemId={managerWorkItemId}
                specialistAgentId={managerSpecialistAgentId}
                steps={managerSteps}
                isSaving={createManagerPlanMutation.isPending}
                error={createManagerPlanMutation.error}
                onGoalChange={setManagerGoal}
                onRiskChange={setManagerRisk}
                onWorkItemChange={setManagerWorkItemId}
                onSpecialistChange={setManagerSpecialistAgentId}
                onStepsChange={setManagerSteps}
                onCreate={() => createManagerPlanMutation.mutate()}
              />
            </Panel>

            <Panel title="Manager plan lifecycle">
              <ManagerPlanConsole
                plans={managerPlans.data ?? []}
                agents={agents.data ?? []}
                workItems={workItems.data ?? []}
                handoffs={agentHandoffs.data ?? []}
                assignments={agentHandoffAssignments.data ?? []}
                isReviewing={reviewManagerPlanMutation.isPending}
                error={reviewManagerPlanMutation.error}
                onReview={(id, status) => reviewManagerPlanMutation.mutate({ id, status })}
              />
            </Panel>
          </div>
          <aside className="product-side">
            <Panel title="Work intake">
              <WorkItemIntakePanel workItems={workItems.data ?? []} />
            </Panel>
            <Panel title="Routing evidence">
              <HandoffEvidencePanel
                handoffs={agentHandoffs.data ?? []}
                assignments={agentHandoffAssignments.data ?? []}
                agents={agents.data ?? []}
                isMutating={handoffTransition.isPending}
                error={handoffTransition.error}
                onTransition={(id, action) => handoffTransition.mutate({ id, action })}
              />
            </Panel>
          </aside>
        </section>
      ) : activeView === "semantic" ? (
        <section className="product-page semantic-page">
          <div className="product-main">
            <Panel title="Memory governance console">
              {memoryGovernance.data ? (
                <MemoryGovernanceConsole
                  summary={memoryGovernance.data}
                  detail={memoryPartition.data}
                  selectedPartitionKey={selectedMemoryPartitionKey}
                  onSelectPartition={setMemoryPartitionKey}
                />
              ) : <p className="muted">Memory governance summary is not loaded.</p>}
            </Panel>
            <Panel title="Semantic graph">
              <OntologyRegistryPanel registry={ontologyRegistry.data} />
              <SemanticObjectBrowser
                objects={visibleSemanticObjects}
                objectTypes={semanticObjectTypes}
                selectedType={semanticObjectType}
                onTypeChange={setSemanticObjectType}
              />
              <SemanticLinkManager
                objects={semanticObjects.data ?? []}
                links={semanticLinks.data ?? []}
                draft={linkDraft}
                registry={ontologyRegistry.data}
                isSaving={createSemanticLink.isPending}
                error={createSemanticLink.error}
                onDraftChange={setLinkDraft}
                onCreate={() => createSemanticLink.mutate()}
              />
            </Panel>
          </div>
          <aside className="product-side">
            <Panel title="Ingestion">
              <SemanticIngestionPanel
                draft={semanticIngestionDraft}
                result={createSemanticIngestionBatch.data}
                isSaving={createSemanticIngestionBatch.isPending}
                error={createSemanticIngestionBatch.error}
                onDraftChange={setSemanticIngestionDraft}
                onCreate={() => createSemanticIngestionBatch.mutate()}
              />
            </Panel>
            <Panel title="Reflection / dreaming">
              <SemanticSynthesisPanel
                sessionId={sessionId}
                draft={semanticSynthesisDraft}
                result={createSemanticSynthesisRun.data}
                isSaving={createSemanticSynthesisRun.isPending}
                error={createSemanticSynthesisRun.error}
                onDraftChange={setSemanticSynthesisDraft}
                onCreate={() => createSemanticSynthesisRun.mutate()}
              />
              <SchedulerPanel summary={schedulerSummary.data ?? emptySchedulerSummary()} />
              <div className="action-row">
                <button disabled={schedulerRunDue.isPending} onClick={() => schedulerRunDue.mutate()}>
                  {schedulerRunDue.isPending ? "Running..." : "Run due jobs"}
                </button>
                <span>scheduler-backed synthesis</span>
              </div>
              {schedulerRunDue.error ? <p className="error-note">{errorMessage(schedulerRunDue.error)}</p> : null}
            </Panel>
            <Panel title="Writeback queue">
              {memoryWritebacks.data ? (
                <MemoryWritebackQueuePanel
                  queue={memoryWritebacks.data}
                  fullCandidates={allWritebackCandidates.data ?? []}
                  status={writebackStatus}
                  limit={writebackLimit}
                  selectedId={selectedQueueWritebackId}
                  reviewReason={writebackReviewReason}
                  isLoadingDetail={allWritebackCandidates.isLoading}
                  isReviewing={reviewWriteback.isPending}
                  error={reviewWriteback.error ?? allWritebackCandidates.error}
                  onStatusChange={(status) => {
                    setWritebackStatus(status);
                    setSelectedQueueWritebackId("");
                  }}
                  onLimitChange={setWritebackLimit}
                  onSelect={setSelectedQueueWritebackId}
                  onReasonChange={setWritebackReviewReason}
                  onReview={(id, decision, reason) => reviewWriteback.mutate({ id, decision, reason })}
                />
              ) : <p className="muted">Writeback queue not loaded.</p>}
            </Panel>
          </aside>
        </section>
      ) : activeView === "packs" ? (
        <section className="product-page packs-page">
          <div className="product-main">
            <Panel title="Pack marketplace">
              <WorkflowPackMarketplace
                manifestPath={packManifestPath}
                installations={workflowPackInstallations.data ?? []}
                selectedInstallationId={packConsoleInstallationId}
                validateResult={packValidate.data}
                installResult={packInstall.data}
                validateError={packValidate.error}
                installError={packInstall.error}
                isValidating={packValidate.isPending}
                isInstalling={packInstall.isPending}
                onManifestPathChange={setPackManifestPath}
                onSelectInstallation={setSelectedPackInstallationId}
                onValidate={() => packValidate.mutate()}
                onInstall={() => packInstall.mutate()}
              />
            </Panel>
            <Panel title="Release console">
              <WorkflowPackReleaseConsole
                installation={selectedPackInstallation}
                onboardingAssessment={packOnboardingAssess.data}
                connectorQuality={packConnectorQualityAssess.data}
                profileAssets={packProfileAssets.data ?? []}
                profileDraft={packProfileDraft}
                releaseEvidence={packReleaseEvidence}
                bindings={packConsoleBindings.data ?? []}
                runtimeObjects={packConsoleRuntimeObjects.data ?? []}
                isAssessing={packOnboardingAssess.isPending}
                isAssessingConnectorQuality={packConnectorQualityAssess.isPending}
                isSavingProfiles={packProfilesSave.isPending}
                isStaging={packStage.isPending}
                isReleasing={packRelease.isPending}
                isRollingBack={packRollback.isPending}
                isUpdating={packUpdate.isPending}
                isArchiving={packArchive.isPending}
                canUpdate={Boolean(packManifestPath.trim())}
                error={packOnboardingAssess.error ?? packConnectorQualityAssess.error ?? packProfilesSave.error ?? packStage.error ?? packRelease.error ?? packRollback.error ?? packUpdate.error ?? packArchive.error}
                onProfileDraftChange={setPackProfileDraft}
                onReleaseEvidenceChange={setPackReleaseEvidence}
                onAssess={() => packOnboardingAssess.mutate()}
                onAssessConnectorQuality={() => packConnectorQualityAssess.mutate()}
                onSaveProfiles={() => packProfilesSave.mutate()}
                onStage={() => packStage.mutate()}
                onRelease={() => packRelease.mutate()}
                onRollback={() => packRollback.mutate()}
                onUpdate={() => packUpdate.mutate()}
                onArchive={() => packArchive.mutate()}
              />
            </Panel>
          </div>
          <aside className="product-side">
            <Panel title="Runtime objects">
              <PackRuntimePanel
                packInstallationId={packConsoleInstallationId || "none"}
                runtimeObjects={packConsoleRuntimeObjects.data ?? []}
                bindings={packConsoleBindings.data ?? []}
                isLoading={packConsoleRuntimeObjects.isLoading || packConsoleBindings.isLoading}
                error={packConsoleRuntimeObjects.error ?? packConsoleBindings.error}
              />
            </Panel>
          </aside>
        </section>
      ) : (
        <section className="product-page deploy-page">
          <div className="product-main">
            <Panel title="Production readiness">
              <DeployReadinessPanel
                stage2={stage2Readiness.data}
                codexDeployment={codexDeploymentReadiness.data}
                observability={observabilitySummary.data}
                errors={[stage2Readiness.error, codexDeploymentReadiness.error, observabilitySummary.error]}
              />
            </Panel>
            <Panel title="Latest version path">
              <LatestDeploymentPanel version={deploymentVersion.data} error={deploymentVersion.error} />
            </Panel>
          </div>
          <aside className="product-side">
            <Panel title="Worker and runtime">
              <KeyValue label="Worker jobs" value={String((sessionLoopJobs.data ?? []).length + (executionJobs.data ?? []).length)} />
              <KeyValue label="Latest job" value={selectedRow?.latestJob?.reason ?? selectedRow?.latestJob?.tool_name ?? "none"} />
              <KeyValue label="Worker" value={selectedRow?.latestJob?.worker_id ?? "waiting"} />
              <KeyValue label="Runtime" value={runtime.execution} />
            </Panel>
            <Panel title="Scheduler">
              {schedulerSummary.data ? (
                <SchedulerPanel summary={schedulerSummary.data} />
              ) : <p className="muted">Scheduler summary is loading.</p>}
            </Panel>
          </aside>
        </section>
      )}

      <footer className="taskbar">
        <select value={selectedAgentId || selectedAgent?.id || ""} onChange={(event) => setSelectedAgentId(event.target.value)}>
          {(agents.data ?? []).map((agent) => <option key={agent.id} value={agent.id}>{agent.name}</option>)}
        </select>
        <select value={selectedEnvironmentId || selectedEnvironment?.id || ""} onChange={(event) => setSelectedEnvironmentId(event.target.value)}>
          {(environments.data ?? []).map((environment) => <option key={environment.id} value={environment.id}>{environment.name}</option>)}
        </select>
        <textarea value={task} onChange={(event) => setTask(event.target.value)} rows={2} placeholder="Describe the job for the managed agent..." />
        <button disabled={!task.trim() || launch.isPending} onClick={() => launch.mutate()}>{launch.isPending ? "Starting..." : "Start task"}</button>
      </footer>
    </main>
  );
}

function ManagerPlanComposer({
  sessions,
  workItems,
  specialists,
  goal,
  risk,
  workItemId,
  specialistAgentId,
  steps,
  isSaving,
  error,
  onGoalChange,
  onRiskChange,
  onWorkItemChange,
  onSpecialistChange,
  onStepsChange,
  onCreate,
}: {
  sessions: SessionRow[];
  workItems: WorkItem[];
  specialists: Agent[];
  goal: string;
  risk: string;
  workItemId: string;
  specialistAgentId: string;
  steps: string;
  isSaving: boolean;
  error: unknown;
  onGoalChange: (value: string) => void;
  onRiskChange: (value: string) => void;
  onWorkItemChange: (value: string) => void;
  onSpecialistChange: (value: string) => void;
  onStepsChange: (value: string) => void;
  onCreate: () => void;
}) {
  return (
    <div className="manager-composer">
      <div className="graph-summary">
        <KeyValue label="Manager sessions" value={String(sessions.length)} />
        <KeyValue label="Work items" value={String(workItems.length)} />
        <KeyValue label="Specialists" value={String(specialists.length)} />
      </div>
      <div className="control-grid manager-grid">
        <label className="wide-filter">
          <span>Goal</span>
          <textarea rows={3} value={goal} onChange={(event) => onGoalChange(event.target.value)} />
        </label>
        <label>
          <span>Work item</span>
          <select value={workItemId} onChange={(event) => onWorkItemChange(event.target.value)}>
            <option value="">No work item</option>
            {workItems.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
          </select>
        </label>
        <label>
          <span>Specialist</span>
          <select value={specialistAgentId} onChange={(event) => onSpecialistChange(event.target.value)}>
            <option value="">Unassigned</option>
            {specialists.map((agent) => <option key={agent.id} value={agent.id}>{agent.name}</option>)}
          </select>
        </label>
        <label>
          <span>Risk</span>
          <select value={risk} onChange={(event) => onRiskChange(event.target.value)}>
            {["low", "medium", "high"].map((value) => <option key={value} value={value}>{value}</option>)}
          </select>
        </label>
        <label className="wide-filter">
          <span>Decomposition</span>
          <textarea rows={5} value={steps} onChange={(event) => onStepsChange(event.target.value)} />
        </label>
      </div>
      <div className="action-row">
        <button disabled={isSaving || !goal.trim() || !sessions.length} onClick={onCreate}>
          {isSaving ? "Creating..." : "Create manager plan"}
        </button>
        <span>{sessions.length ? "records manager_plan.created into audit" : "create or select a manager session first"}</span>
      </div>
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
    </div>
  );
}

function ManagerPlanConsole({
  plans,
  agents,
  workItems,
  handoffs,
  assignments,
  isReviewing,
  error,
  onReview,
}: {
  plans: ManagerAgentPlan[];
  agents: Agent[];
  workItems: WorkItem[];
  handoffs: AgentHandoffEvent[];
  assignments: AgentHandoffAssignment[];
  isReviewing: boolean;
  error: unknown;
  onReview: (id: string, status: string) => void;
}) {
  const sorted = [...plans].sort((left, right) => right.created_at.localeCompare(left.created_at));
  return (
    <div className="manager-console">
      <div className="graph-summary">
        <KeyValue label="Plans" value={String(plans.length)} />
        <KeyValue label="Handoffs" value={String(handoffs.length)} />
        <KeyValue label="Assignments" value={String(assignments.length)} />
      </div>
      {sorted.length ? sorted.map((plan) => {
        const planHandoffs = handoffs.filter((handoff) => handoff.manager_plan_id === plan.id);
        return (
          <article key={plan.id} className="manager-plan-card">
            <div className="inspection-title">
              <strong>{planGoal(plan)}</strong>
              <span>{plan.status} · {plan.risk_classification}</span>
            </div>
            <div className="field-grid">
              <KeyValue label="Manager" value={agentName(agents, plan.manager_agent_id)} />
              <KeyValue label="Specialist" value={agentName(agents, plan.specialist_agent_id)} />
              <KeyValue label="Work item" value={workItemTitle(workItems, plan.work_item_id)} />
              <KeyValue label="Plan ID" value={shortId(plan.id)} />
              <KeyValue label="Updated" value={relativeAge(plan.updated_at)} />
              <KeyValue label="Handoffs" value={String(planHandoffs.length)} />
            </div>
            <div className="scope-line compact">
              {planSteps(plan).slice(0, 6).map((step) => <span key={step}>{step}</span>)}
              {!planSteps(plan).length ? <span>no decomposition steps</span> : null}
            </div>
            <div className="review-actions">
              <button disabled={isReviewing || plan.status === "approved"} onClick={() => onReview(plan.id, "approved")}>Approve</button>
              <button disabled={isReviewing} className="ghost" onClick={() => onReview(plan.id, "needs_changes")}>Needs changes</button>
              <button disabled={isReviewing} className="ghost danger" onClick={() => onReview(plan.id, "blocked")}>Block</button>
            </div>
            {planHandoffs.length ? (
              <div className="handoff-strip">
                {planHandoffs.map((handoff) => (
                  <span key={handoff.id}>{handoff.intent} · {handoff.status} · {handoff.review_status}</span>
                ))}
              </div>
            ) : null}
          </article>
        );
      }) : <p className="muted">No manager plans yet.</p>}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
    </div>
  );
}

function WorkItemIntakePanel({ workItems }: { workItems: WorkItem[] }) {
  const sorted = [...workItems].sort((left, right) => right.updated_at.localeCompare(left.updated_at));
  return (
    <div className="work-item-list">
      {sorted.slice(0, 12).map((item) => (
        <div key={item.id} className="obs-row">
          <strong>{item.title}</strong>
          <span>{item.status} · {item.priority} · {item.source}</span>
          <small>{item.description ?? shortId(item.id)}</small>
        </div>
      ))}
      {!sorted.length ? <p className="muted">No WorkItems are available yet.</p> : null}
    </div>
  );
}

function HandoffEvidencePanel({
  handoffs,
  assignments,
  agents,
  isMutating,
  error,
  onTransition,
}: {
  handoffs: AgentHandoffEvent[];
  assignments: AgentHandoffAssignment[];
  agents: Agent[];
  isMutating: boolean;
  error: unknown;
  onTransition: (id: string, action: "accept" | "reject" | "fail" | "complete" | "escalate") => void;
}) {
  return (
    <div className="handoff-evidence">
      {handoffs.slice(-10).reverse().map((handoff) => {
        const assignment = assignments.find((item) => item.agent_handoff_event_id === handoff.id);
        return (
          <div key={handoff.id} className="obs-row">
            <strong>{handoff.intent} · {handoff.status}</strong>
            <span>{agentName(agents, handoff.source_agent_id)} to {agentName(agents, handoff.target_agent_id)}</span>
            <small>{handoff.review_status} · escalation {handoff.human_escalation_status} · assignment {assignment?.status ?? "none"}</small>
            <div className="review-actions compact-actions">
              <button disabled={isMutating || handoff.status !== "requested"} onClick={() => onTransition(handoff.id, "accept")}>Accept</button>
              <button disabled={isMutating || handoff.status !== "accepted"} onClick={() => onTransition(handoff.id, "complete")}>Complete</button>
              <button disabled={isMutating || handoff.status === "completed" || handoff.status === "rejected"} className="ghost" onClick={() => onTransition(handoff.id, "escalate")}>Escalate</button>
              <button disabled={isMutating || handoff.status === "completed" || handoff.status === "rejected"} className="ghost danger" onClick={() => onTransition(handoff.id, "fail")}>Fail</button>
            </div>
          </div>
        );
      })}
      {!handoffs.length ? <p className="muted">No handoff events recorded yet.</p> : null}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
    </div>
  );
}

function MemoryGovernanceConsole({
  summary,
  detail,
  selectedPartitionKey,
  onSelectPartition,
}: {
  summary: MemoryGovernanceSummary;
  detail?: MemoryGovernancePartitionDetail;
  selectedPartitionKey: string;
  onSelectPartition: (key: string) => void;
}) {
  return (
    <div className="memory-console">
      <div className="graph-summary">
        <KeyValue label="Status" value={summary.status} />
        <KeyValue label="Isolation" value={summary.isolation_policy} />
        <KeyValue label="Partitions" value={String(summary.partition_count)} />
        <KeyValue label="Pending writebacks" value={String(summary.writeback.pending_count)} />
      </div>
      <div className="partition-list">
        {summary.partitions.map((partition) => (
          <button
            key={partition.partition_key}
            className={selectedPartitionKey === partition.partition_key ? "partition-chip selected" : "partition-chip"}
            onClick={() => onSelectPartition(partition.partition_key)}
          >
            <span>{partition.domain_scope}/{partition.workflow_scope}</span>
            <strong>{partition.memory_object_count}</strong>
          </button>
        ))}
      </div>
      {detail ? <MemoryPartitionDetail detail={detail} /> : <p className="muted">Choose a partition to inspect governance detail.</p>}
      {summary.attention_items.map((item) => (
        <Row key={`${item.kind}-${item.message}`} title={`${item.severity} · ${item.kind}`} detail={item.message} />
      ))}
    </div>
  );
}

function WorkflowPackMarketplace({
  manifestPath,
  installations,
  selectedInstallationId,
  validateResult,
  installResult,
  validateError,
  installError,
  isValidating,
  isInstalling,
  onManifestPathChange,
  onSelectInstallation,
  onValidate,
  onInstall,
}: {
  manifestPath: string;
  installations: WorkflowPackInstallation[];
  selectedInstallationId: string;
  validateResult?: Record<string, unknown>;
  installResult?: WorkflowPackInstallation;
  validateError: unknown;
  installError: unknown;
  isValidating: boolean;
  isInstalling: boolean;
  onManifestPathChange: (value: string) => void;
  onSelectInstallation: (id: string) => void;
  onValidate: () => void;
  onInstall: () => void;
}) {
  return (
    <div className="pack-marketplace">
      <div className="control-grid">
        <label className="wide-filter">
          <span>Manifest path</span>
          <input value={manifestPath} onChange={(event) => onManifestPathChange(event.target.value)} />
        </label>
        <button disabled={isValidating || !manifestPath.trim()} onClick={onValidate}>{isValidating ? "Validating..." : "Validate"}</button>
        <button disabled={isInstalling || !manifestPath.trim()} onClick={onInstall}>{isInstalling ? "Installing..." : "Install"}</button>
      </div>
      <div className="graph-summary">
        <KeyValue label="Installed packs" value={String(installations.length)} />
        <KeyValue label="Selected" value={shortId(selectedInstallationId || "none")} />
        <KeyValue label="Last install" value={installResult ? `${installResult.pack_id}@${installResult.version}` : "none"} />
      </div>
      {validateResult ? (
        <details className="json-details" open>
          <summary>Validation report</summary>
          <pre>{formatJson(validateResult)}</pre>
        </details>
      ) : null}
      <div className="installation-list">
        {installations.map((installation) => (
          <button
            key={installation.id}
            className={installation.id === selectedInstallationId ? "installation-row selected" : "installation-row"}
            onClick={() => onSelectInstallation(installation.id)}
          >
            <strong>{installation.pack_id}@{installation.version}</strong>
            <span>{installation.kind} · {installation.status} · {relativeAge(installation.updated_at)}</span>
          </button>
        ))}
      </div>
      {validateError ? <p className="error-note">{errorMessage(validateError)}</p> : null}
      {installError ? <p className="error-note">{errorMessage(installError)}</p> : null}
    </div>
  );
}

function WorkflowPackReleaseConsole({
  installation,
  onboardingAssessment,
  connectorQuality,
  profileAssets,
  profileDraft,
  releaseEvidence,
  bindings,
  runtimeObjects,
  isAssessing,
  isAssessingConnectorQuality,
  isSavingProfiles,
  isStaging,
  isReleasing,
  isRollingBack,
  isUpdating,
  isArchiving,
  canUpdate,
  error,
  onProfileDraftChange,
  onReleaseEvidenceChange,
  onAssess,
  onAssessConnectorQuality,
  onSaveProfiles,
  onStage,
  onRelease,
  onRollback,
  onUpdate,
  onArchive,
}: {
  installation?: WorkflowPackInstallation;
  onboardingAssessment?: WorkflowPackOnboardingAssessment;
  connectorQuality?: WorkflowPackConnectorQualityAssessment;
  profileAssets: WorkflowPackProfileAsset[];
  profileDraft: string;
  releaseEvidence: string;
  bindings: WorkflowPackBinding[];
  runtimeObjects: WorkflowPackRuntimeObject[];
  isAssessing: boolean;
  isAssessingConnectorQuality: boolean;
  isSavingProfiles: boolean;
  isStaging: boolean;
  isReleasing: boolean;
  isRollingBack: boolean;
  isUpdating: boolean;
  isArchiving: boolean;
  canUpdate: boolean;
  error: unknown;
  onProfileDraftChange: (value: string) => void;
  onReleaseEvidenceChange: (value: string) => void;
  onAssess: () => void;
  onAssessConnectorQuality: () => void;
  onSaveProfiles: () => void;
  onStage: () => void;
  onRelease: () => void;
  onRollback: () => void;
  onUpdate: () => void;
  onArchive: () => void;
}) {
  if (!installation) {
    return <p className="muted">Install or select a workflow pack first.</p>;
  }
  return (
    <div className="pack-release-console">
      <div className="graph-summary">
        <KeyValue label="Pack" value={`${installation.pack_id}@${installation.version}`} />
        <KeyValue label="Status" value={installation.status} />
        <KeyValue label="Profiles" value={String(profileAssets.length)} />
        <KeyValue label="Bindings" value={String(bindings.length)} />
        <KeyValue label="Runtime objects" value={String(runtimeObjects.length)} />
      </div>
      <textarea rows={7} value={profileDraft} spellCheck={false} onChange={(event) => onProfileDraftChange(event.target.value)} />
      <div className="action-row">
        <button disabled={isSavingProfiles} onClick={onSaveProfiles}>{isSavingProfiles ? "Saving..." : "Save profiles"}</button>
        <button disabled={isAssessing} onClick={onAssess}>{isAssessing ? "Assessing..." : "Assess onboarding"}</button>
        <button disabled={isAssessingConnectorQuality} onClick={onAssessConnectorQuality}>{isAssessingConnectorQuality ? "Checking..." : "Assess connectors"}</button>
        <span>{onboardingAssessment ? `${onboardingAssessment.status} · ${onboardingAssessment.blockers.length} blockers` : "assessment pending"}</span>
      </div>
      {onboardingAssessment ? (
        <div className={onboardingAssessment.status === "ready" ? "readiness-card ready" : "readiness-card blocked"}>
          <strong>{onboardingAssessment.status}</strong>
          <span>{onboardingAssessment.ready_connector_count}/{onboardingAssessment.connector_requirement_count} connectors ready · {onboardingAssessment.placeholder_profile_count} placeholder profiles</span>
          {onboardingAssessment.blockers.slice(0, 6).map((blocker) => <small key={blocker}>{blocker}</small>)}
        </div>
      ) : null}
      {connectorQuality ? (
        <div className={connectorQuality.status === "ready" ? "readiness-card ready" : "readiness-card blocked"}>
          <strong>Connector quality · {connectorQuality.status}</strong>
          <span>{connectorQuality.ready_connector_count}/{connectorQuality.connector_requirement_count} connectors ready</span>
          {connectorQuality.blockers.slice(0, 6).map((blocker) => <small key={blocker}>{blocker}</small>)}
        </div>
      ) : null}
      <textarea rows={5} value={releaseEvidence} spellCheck={false} onChange={(event) => onReleaseEvidenceChange(event.target.value)} />
      <div className="review-actions">
        <button disabled={isStaging || installation.status !== "installed"} onClick={onStage}>{isStaging ? "Staging..." : "Stage"}</button>
        <button disabled={isReleasing || installation.status !== "staged"} onClick={onRelease}>{isReleasing ? "Releasing..." : "Release"}</button>
        <button disabled={isRollingBack || installation.status !== "released"} className="ghost danger" onClick={onRollback}>{isRollingBack ? "Rolling back..." : "Rollback"}</button>
        <button disabled={isUpdating || !canUpdate || !["released", "rolled_back"].includes(installation.status)} className="ghost" onClick={onUpdate}>{isUpdating ? "Updating..." : "Create update"}</button>
        <button disabled={isArchiving} className="ghost danger" onClick={onArchive}>{isArchiving ? "Archiving..." : "Archive"}</button>
      </div>
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
    </div>
  );
}

function DeployReadinessPanel({
  stage2,
  codexDeployment,
  observability,
  errors,
}: {
  stage2?: Stage2Readiness;
  codexDeployment?: Record<string, unknown>;
  observability?: ObservabilitySummary;
  errors: unknown[];
}) {
  return (
    <div className="deploy-readiness">
      <div className="graph-summary">
        <KeyValue label="Stage 2" value={readinessStatus(stage2)} />
        <KeyValue label="Codex deploy" value={recordStatus(codexDeployment)} />
        <KeyValue label="Observability" value={recordStatus(observability)} />
      </div>
      {stage2 ? <ReadinessRecord title="Stage 2 readiness" record={stage2} /> : <p className="muted">Stage 2 readiness is loading.</p>}
      {codexDeployment ? <ReadinessRecord title="Codex App Server deployment" record={codexDeployment} /> : null}
      {observability ? <ReadinessRecord title="Observability" record={observability} /> : null}
      {errors.filter(Boolean).map((error, index) => <p key={index} className="error-note">{errorMessage(error)}</p>)}
    </div>
  );
}

function ReadinessRecord({ title, record }: { title: string; record: Record<string, unknown> }) {
  return (
    <details className="json-details">
      <summary>{title}</summary>
      <pre>{formatJson(record)}</pre>
    </details>
  );
}

function LatestDeploymentPanel({ version, error }: { version?: DeploymentVersion; error?: unknown }) {
  const imageTag = version?.image_tag || "not reported";
  const gitSha = version?.git_sha || "not reported";
  const buildTime = version?.build_time || "not reported";
  return (
    <div className="latest-deploy-panel">
      <div className={`readiness-card ${version?.image_tag ? "ready" : "blocked"}`}>
        <strong>{version ? "Running API version" : "Running API version unavailable"}</strong>
        <span>{version ? `${version.service} · ${version.source}` : "Waiting for /api/deployment/version."}</span>
        {error ? <small>{errorMessage(error)}</small> : null}
      </div>
      <div className="deploy-version-grid">
        <KeyValue label="Image tag" value={imageTag} />
        <KeyValue label="Git SHA" value={shortId(gitSha)} />
        <KeyValue label="Build time" value={buildTime} />
        <KeyValue label="Cargo version" value={version?.cargo_package_version ?? "not reported"} />
      </div>
      <pre>{[
        "curl -fsS http://127.0.0.1:8787/api/deployment/version",
        "npm run build --prefix web-ui",
        "scripts/verify-static-ui-assets.sh",
        "git diff --check",
        "gh workflow run Deploy -f image_tag=whiskey-$(git rev-parse --short HEAD) -f publish_image=true -f deploy_whiskey=true",
        "ssh wishky-2-1 'cat /opt/mandoforge-adoption/evidence/deployment-version.json'",
        "scripts/whiskey-adoption-evidence.sh",
      ].join("\n")}</pre>
    </div>
  );
}

function buildRows(input: {
  sessions: Session[];
  agents: Agent[];
  approvals: Approval[];
  jobs: WorkerJob[];
  toolCalls: ToolCall[];
  workflowRuns: WorkflowRun[];
}): SessionRow[] {
  return [...input.sessions]
    .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
    .map((session) => {
      const workflowRun = input.workflowRuns.find((run) => run.primary_session_id === session.id);
      const sessionApprovals = input.approvals.filter((approval) => approval.session_id === session.id && approval.status === "pending");
      const jobs = input.jobs
        .filter((job) => job.session_id === session.id)
        .sort((left, right) => timestamp(right).localeCompare(timestamp(left)));
      const latestJob = jobs[0];
      const toolCount = input.toolCalls.filter((call) => call.session_id === session.id).length;
      const status = deriveRowStatus(session, sessionApprovals, latestJob, toolCount);
      return {
        session,
        agent: input.agents.find((agent) => agent.id === session.agent_id),
        workflowRun,
        status,
        label: labelForStatus(status),
        detail: detailForStatus(status, latestJob, toolCount, workflowRun),
        latestJob,
        toolCount,
        approvalCount: sessionApprovals.length,
        updatedAt: latestJob?.completed_at ?? latestJob?.started_at ?? latestJob?.enqueued_at ?? session.updated_at,
      };
    });
}

function deriveRowStatus(session: Session, approvals: Approval[], latestJob: WorkerJob | undefined, toolCount: number): RowStatus {
  if (session.status === "failed" || latestJob?.status === "failed") return "failed";
  if (approvals.length) return "needs_input";
  if (latestJob?.status === "running" || session.status === "running") return "running";
  if (latestJob?.status === "queued") return "queued";
  if (latestJob?.status === "completed" || toolCount > 0) return "completed";
  return "idle";
}

function detailForStatus(status: RowStatus, job: WorkerJob | undefined, toolCount: number, workflowRun?: WorkflowRun): string {
  if (status === "running") return `${job?.worker_id ?? "worker"} processing`;
  if (status === "queued") return "waiting for worker";
  if (status === "needs_input") return "waiting for approval";
  if (workflowRun) return `workflow ${workflowRun.status}`;
  if (status === "completed") return `${toolCount} tool calls completed`;
  if (status === "failed") return job?.last_error ?? "failed";
  return "ready";
}

function labelForStatus(status: RowStatus): string {
  return {
    needs_input: "Needs input",
    running: "Running",
    queued: "Queued",
    completed: "Completed",
    failed: "Failed",
    idle: "Idle",
  }[status];
}

function StatusLogo({ status }: { status: RowStatus }) {
  return <span className={`status-logo logo-${status}`} aria-label={status} />;
}

function Metric({ label, value, tone }: { label: string; value: string; tone?: "live" | "warn" }) {
  return <article className={tone ? `metric metric-${tone}` : "metric"}><span>{label}</span><strong>{value}</strong></article>;
}

function Panel({ title, children }: { title: string; children: React.ReactNode }) {
  return <section className="obs-card"><h2>{title}</h2>{children}</section>;
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return <div className="kv"><span>{label}</span><strong>{value}</strong></div>;
}

function Row({ title, detail }: { title: string; detail: string }) {
  return <div className="obs-row"><strong>{title}</strong><span>{detail}</span></div>;
}

function StepRow({ step }: { step: WorkflowStepRun }) {
  return (
    <div className="obs-row step-row">
      <StatusLogo status={statusFromText(step.status)} />
      <div>
        <strong>{step.step_key}</strong>
        <span>
          {step.step_type} · {step.status}
          {step.claimed_by_worker ? ` · ${step.claimed_by_worker}` : ""}
          {step.context_packet_id ? ` · ctx ${shortId(step.context_packet_id)}` : ""}
          {step.scheduled_at ? ` · due ${relativeAge(step.scheduled_at)}` : ""}
        </span>
      </div>
    </div>
  );
}

function TaskBoardPanel({
  board,
  agents,
  selectedAgent,
  isRunning,
  runError,
  onRun,
}: {
  board: TaskBoardSnapshot;
  agents: Agent[];
  selectedAgent?: Agent;
  isRunning: boolean;
  runError: unknown;
  onRun: (stepId: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [columnFilter, setColumnFilter] = useState<TaskBoardColumnId | "all">("all");
  const [agentFilter, setAgentFilter] = useState("all");
  const [onlyClaimable, setOnlyClaimable] = useState(false);
  const [selectedItemId, setSelectedItemId] = useState("");

  const agentOptions = useMemo(() => {
    const ids = new Set(board.items.map((item) => item.agent_id).filter((id): id is string => Boolean(id)));
    return agents.filter((agent) => ids.has(agent.id));
  }, [agents, board.items]);

  const filteredItems = useMemo(() => {
    return [...board.items]
      .filter((item) => columnFilter === "all" || taskBoardColumnFor(item) === columnFilter)
      .filter((item) => agentFilter === "all" || (agentFilter === "unassigned" ? !item.agent_id : item.agent_id === agentFilter))
      .filter((item) => !onlyClaimable || item.claimable)
      .filter((item) => matchesTaskBoardQuery(item, agents, query))
      .sort(compareTaskBoardItems);
  }, [agents, agentFilter, board.items, columnFilter, onlyClaimable, query]);

  const columns = useMemo(() => {
    return TASK_BOARD_COLUMNS.map((column) => ({
      ...column,
      items: filteredItems.filter((item) => taskBoardColumnFor(item) === column.id),
    }));
  }, [filteredItems]);

  const activeItem = filteredItems.find((item) => item.workflow_step_run_id === selectedItemId) ?? filteredItems[0] ?? null;
  const activeItemId = activeItem?.workflow_step_run_id ?? "";
  const generatedAge = relativeAge(board.generated_at);

  return (
    <div className="task-board-panel task-board-workbench">
      <div className="task-board-toolbar">
        <div>
          <strong>Collaboration kanban</strong>
          <span>{board.items.length} live cards · refreshed {generatedAge ? `${generatedAge} ago` : "now"}</span>
        </div>
        <span className="operator-pill">Run as {selectedAgent?.name ?? "no agent"}</span>
      </div>

      <div className="graph-summary board-summary">
        <KeyValue label="Work items" value={String(board.work_item_count)} />
        <KeyValue label="Workflows" value={String(board.workflow_run_count)} />
        <KeyValue label="Steps" value={String(board.workflow_step_count)} />
        <KeyValue label="Claimable" value={String(board.claimable_count)} />
        <KeyValue label="Visible" value={String(filteredItems.length)} />
        <KeyValue label="Updated" value={generatedAge ? `${generatedAge} ago` : "now"} />
      </div>

      <div className="task-board-filters">
        <label className="wide-filter">
          <span>Search</span>
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="work item, step, agent, status, id" />
        </label>
        <label>
          <span>Column</span>
          <select value={columnFilter} onChange={(event) => setColumnFilter(event.target.value as TaskBoardColumnId | "all")}>
            <option value="all">All columns</option>
            {TASK_BOARD_COLUMNS.map((column) => <option key={column.id} value={column.id}>{column.title}</option>)}
          </select>
        </label>
        <label>
          <span>Agent</span>
          <select value={agentFilter} onChange={(event) => setAgentFilter(event.target.value)}>
            <option value="all">All agents</option>
            <option value="unassigned">Unassigned</option>
            {agentOptions.map((agent) => <option key={agent.id} value={agent.id}>{agent.name}</option>)}
          </select>
        </label>
        <label className="checkbox-control">
          <input type="checkbox" checked={onlyClaimable} onChange={(event) => setOnlyClaimable(event.target.checked)} />
          <span>Claimable only</span>
        </label>
      </div>

      <div className="status-counts">
        {Object.entries(board.status_counts).map(([status, count]) => (
          <span key={status} className={`status-count node-${statusFromText(status)}`}>{status}: {count}</span>
        ))}
      </div>

      {runError ? <p className="error-note">Run failed: {errorMessage(runError)}</p> : null}

      <div className="task-kanban" aria-label="Collaboration task board">
        {columns.map((column) => (
          <section key={column.id} className={`task-column task-column-${column.id}`}>
            <div className="task-column-head">
              <div>
                <strong>{column.title}</strong>
                <span>{column.hint}</span>
              </div>
              <b>{column.items.length}</b>
            </div>
            <div className="task-card-stack">
              {column.items.length ? column.items.map((item) => (
                <TaskBoardCard
                  key={item.workflow_step_run_id}
                  item={item}
                  agents={agents}
                  selected={item.workflow_step_run_id === activeItemId}
                  selectedAgent={selectedAgent}
                  isRunning={isRunning}
                  onSelect={() => setSelectedItemId(item.workflow_step_run_id)}
                  onRun={() => onRun(item.workflow_step_run_id)}
                />
              )) : <p className="empty-column">No cards</p>}
            </div>
          </section>
        ))}
      </div>

      {activeItem ? (
        <TaskBoardInspector
          item={activeItem}
          agents={agents}
          selectedAgent={selectedAgent}
          isRunning={isRunning}
          onRun={() => onRun(activeItem.workflow_step_run_id)}
        />
      ) : <p className="muted">No board item matches the current filters.</p>}
    </div>
  );
}

function TaskBoardCard({
  item,
  agents,
  selected,
  selectedAgent,
  isRunning,
  onSelect,
  onRun,
}: {
  item: TaskBoardItemView;
  agents: Agent[];
  selected: boolean;
  selectedAgent?: Agent;
  isRunning: boolean;
  onSelect: () => void;
  onRun: () => void;
}) {
  const column = taskBoardColumnFor(item);
  const statusTone = statusFromText(item.status);
  const runBlocker = taskBoardRunBlocker(item, selectedAgent, agents);
  return (
    <article
      className={selected ? `task-card task-card-${column} task-status-${statusTone} selected` : `task-card task-card-${column} task-status-${statusTone}`}
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") onSelect();
      }}
    >
      <div className="task-card-top">
        <StatusLogo status={statusFromText(item.status)} />
        <div>
          <strong>{item.work_item_title ?? item.step_key}</strong>
          <span>{item.step_key} · {item.step_type}</span>
        </div>
      </div>
      <div className="task-card-meta">
        <span>{item.status}</span>
        <span>{item.work_item_priority ?? "normal"}</span>
        <span>{agentName(agents, item.agent_id)}</span>
      </div>
      <div className="chip-row">
        {item.context_packet_id ? <span className="board-chip">ctx {shortId(item.context_packet_id)}</span> : null}
        {item.task_grant_id ? <span className="board-chip">grant {shortId(item.task_grant_id)}</span> : null}
        {item.claimed_by_worker ? <span className="board-chip">worker {item.claimed_by_worker}</span> : null}
      </div>
      {item.blockers.length ? (
        <small className="task-card-blocker">{item.blockers.slice(0, 2).join(" · ")}{item.blockers.length > 2 ? ` · +${item.blockers.length - 2}` : ""}</small>
      ) : <small className="task-ready-note">No blocker reported</small>}
      <div className="task-card-actions">
        <span>{relativeAge(item.updated_at) || "now"} ago</span>
        <button disabled={Boolean(runBlocker) || isRunning} onClick={(event) => {
          event.stopPropagation();
          onRun();
        }}>
          {isRunning ? "Running..." : "Run"}
        </button>
      </div>
      {runBlocker ? <small className="run-blocker">{runBlocker}</small> : null}
    </article>
  );
}

function TaskBoardInspector({
  item,
  agents,
  selectedAgent,
  isRunning,
  onRun,
}: {
  item: TaskBoardItemView;
  agents: Agent[];
  selectedAgent?: Agent;
  isRunning: boolean;
  onRun: () => void;
}) {
  const runBlocker = taskBoardRunBlocker(item, selectedAgent, agents);
  return (
    <section className="task-inspector">
      <div className="task-inspector-head">
        <div>
          <span>Selected card</span>
          <strong>{item.work_item_title ?? item.step_key}</strong>
        </div>
        <button disabled={Boolean(runBlocker) || isRunning} onClick={onRun}>
          {isRunning ? "Running..." : "Run step"}
        </button>
      </div>
      <div className="task-detail-grid">
        <KeyValue label="Status" value={item.status} />
        <KeyValue label="Column" value={taskBoardColumnFor(item)} />
        <KeyValue label="Agent" value={agentName(agents, item.agent_id)} />
        <KeyValue label="Priority" value={item.work_item_priority ?? "normal"} />
        <KeyValue label="Workflow" value={shortId(item.workflow_run_id)} />
        <KeyValue label="Definition" value={shortId(item.workflow_definition_id)} />
        <KeyValue label="Step run" value={shortId(item.workflow_step_run_id)} />
        <KeyValue label="Task grant" value={shortId(item.task_grant_id ?? "none")} />
        <KeyValue label="Context packet" value={shortId(item.context_packet_id ?? "none")} />
        <KeyValue label="Claimed worker" value={item.claimed_by_worker ?? "none"} />
        <KeyValue label="Lease" value={item.lease_expires_at ? relativeMoment(item.lease_expires_at) : "none"} />
        <KeyValue label="Updated" value={relativeAge(item.updated_at) || "now"} />
      </div>
      {runBlocker ? <p className="status-hint">Run blocked: {runBlocker}</p> : <p className="status-hint">This card can be claimed by the selected agent.</p>}
      <div className="blocker-list">
        {item.blockers.length ? item.blockers.map((blocker) => <span key={blocker}>{blocker}</span>) : <span>No blockers</span>}
      </div>
    </section>
  );
}

function AgentInboxPanel({
  inbox,
  agent,
  isRunning,
  onRun,
}: {
  inbox: AgentInboxSnapshot;
  agent?: Agent;
  isRunning: boolean;
  onRun: (stepId: string) => void;
}) {
  return (
    <div className="agent-inbox-panel">
      <div className="graph-summary">
        <KeyValue label="Agent" value={agent?.name ?? shortId(inbox.agent_id)} />
        <KeyValue label="Entries" value={String(inbox.entry_count)} />
        <KeyValue label="Ready" value={String(inbox.claimable_count)} />
      </div>
      {inbox.entries.map((entry) => (
        <div key={entry.workflow_step_run_id} className={entry.claimable ? "inbox-entry claimable" : "inbox-entry"}>
          <div className="inbox-main">
            <StatusLogo status={statusFromText(entry.status)} />
            <div>
              <strong>{entry.work_item?.title ?? entry.step_key}</strong>
              <span>
                {entry.step_type} · {entry.status}
                {entry.claimed_by_worker ? ` · ${entry.claimed_by_worker}` : ""}
                {entry.context_packet_id ? ` · ctx ${shortId(entry.context_packet_id)}` : ""}
              </span>
              {entry.blockers.length ? <small>{entry.blockers.join(", ")}</small> : <small>worker will bind context and execute</small>}
            </div>
          </div>
          <button disabled={!entry.claimable || isRunning} onClick={() => onRun(entry.workflow_step_run_id)}>
            Run
          </button>
        </div>
      ))}
      {!inbox.entries.length ? <p className="muted">No open work is routed to this agent.</p> : null}
    </div>
  );
}

function WorkflowDefinitionEditor({
  definition,
  draft,
  statusMessage,
  isSaving,
  error,
  onDraftChange,
  onApply,
  onReset,
}: {
  definition?: WorkflowDefinition;
  draft: string;
  statusMessage: string;
  isSaving: boolean;
  error: unknown;
  onDraftChange: (value: string) => void;
  onApply: () => void;
  onReset: () => void;
}) {
  if (!definition) return <p className="muted">No workflow definition selected.</p>;
  return (
    <div className="graph-editor">
      <div className="graph-summary">
        <KeyValue label="Definition" value={shortId(definition.id)} />
        <KeyValue label="Release" value={definition.release_state} />
        <KeyValue label="Updated" value={relativeAge(definition.updated_at)} />
      </div>
      <label className="graph-editor-field">
        <span>step_graph JSON</span>
        <textarea value={draft} onChange={(event) => onDraftChange(event.target.value)} spellCheck={false} />
      </label>
      <div className="action-row">
        <button disabled={isSaving} onClick={onApply}>{isSaving ? "Saving" : "Apply"}</button>
        <button className="secondary" disabled={isSaving} onClick={onReset}>Reset</button>
      </div>
      {statusMessage ? <p className="status-hint">{statusMessage}</p> : null}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
      <details className="json-details">
        <summary>handoff_rules</summary>
        <pre>{formatJson(definition.handoff_rules)}</pre>
      </details>
    </div>
  );
}

function WorkflowGraph({ graph }: { graph: WorkflowGraphConsole }) {
  return (
    <div className="graph-console">
      <div className="graph-summary">
        <KeyValue label="Nodes" value={String(graph.node_count)} />
        <KeyValue label="Edges" value={String(graph.edge_count)} />
        <KeyValue label="Due scheduled" value={String(graph.due_scheduled_count)} />
      </div>
      <div className="status-counts">
        {Object.entries(graph.status_counts).map(([status, count]) => (
          <span key={status} className={`status-count node-${statusFromText(status)}`}>{status}: {count}</span>
        ))}
      </div>
      <div className="graph-nodes">
        {graph.nodes.map((node) => (
          <div key={node.id} className={`graph-node node-${statusFromText(node.status)} ${node.due ? "node-due" : ""}`}>
            <StatusLogo status={statusFromText(node.status)} />
            <div>
              <strong>{node.step_key}</strong>
              <span>
                {node.step_type} · {node.declared ? "declared" : node.status}
                {node.dependencies.length ? ` · after ${node.dependencies.join(", ")}` : ""}
                {node.claimed_by_worker ? ` · ${node.claimed_by_worker}` : ""}
                {node.context_packet_id ? ` · ctx ${shortId(node.context_packet_id)}` : ""}
                {node.scheduled_at ? ` · ${node.due ? "due now" : `due ${relativeAge(node.scheduled_at)}`}` : ""}
              </span>
              <small>{summaryLine(node.output_summary) || summaryLine(node.input_summary) || summaryLine(node.definition_summary)}</small>
            </div>
          </div>
        ))}
      </div>
      <div className="graph-edges">
        {graph.edges.slice(-8).map((edge) => (
          <span key={edge.id}>
            {(edge.from_step_key ?? "start")} {"->"} {(edge.to_step_key ?? "run")} · {edge.declared ? "declared" : edge.transition_type}/{edge.status}
          </span>
        ))}
      </div>
    </div>
  );
}

function TransitionRow({ transition }: { transition: WorkflowTransition }) {
  const from = transition.from_step_key ?? "start";
  const to = transition.to_step_key ?? "run";
  return (
    <div className="obs-row transition-row">
      <strong>{transition.transition_type}</strong>
      <span>{from} {"->"} {to} · {transition.status}</span>
    </div>
  );
}

function PackRuntimePanel({
  packInstallationId,
  runtimeObjects,
  bindings,
  isLoading,
  error,
}: {
  packInstallationId: string;
  runtimeObjects: WorkflowPackRuntimeObject[];
  bindings: WorkflowPackBinding[];
  isLoading: boolean;
  error: unknown;
}) {
  return (
    <div className="pack-runtime">
      <div className="graph-summary">
        <KeyValue label="Installation" value={shortId(packInstallationId)} />
        <KeyValue label="Runtime objects" value={String(runtimeObjects.length)} />
        <KeyValue label="Bindings" value={String(bindings.length)} />
      </div>
      {isLoading ? <p className="muted">Loading pack binding and runtime object state.</p> : null}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
      <div className="inspection-section">
        <strong>Runtime objects</strong>
        {runtimeObjects.length ? runtimeObjects.map((object) => (
          <RuntimeObjectRow key={object.id} object={object} />
        )) : <p className="muted">No runtime objects materialized by this pack installation.</p>}
      </div>
      <div className="inspection-section">
        <strong>Bindings</strong>
        {bindings.length ? bindings.map((binding) => (
          <BindingRow key={binding.id} binding={binding} />
        )) : <p className="muted">No workflow pack bindings returned for this installation.</p>}
      </div>
    </div>
  );
}

function RuntimeObjectRow({ object }: { object: WorkflowPackRuntimeObject }) {
  return (
    <div className="inspection-row runtime-row">
      <div className="inspection-title">
        <strong>{object.object_type} · {object.status}</strong>
        <span>{object.object_key}</span>
      </div>
      <div className="field-grid">
        <KeyValue label="Object ID" value={shortId(object.id)} />
        <KeyValue label="Binding ID" value={shortId(object.binding_id)} />
        <KeyValue label="Runtime kind" value={object.runtime_kind} />
        <KeyValue label="Pack" value={`${object.pack_id}@${object.pack_version}`} />
        <KeyValue label="Created" value={object.created_at} />
        <KeyValue label="Updated" value={object.updated_at} />
      </div>
      <details className="json-details">
        <summary>Spec</summary>
        <pre>{formatJson(object.spec)}</pre>
      </details>
    </div>
  );
}

function BindingRow({ binding }: { binding: WorkflowPackBinding }) {
  return (
    <div className="inspection-row binding-row">
      <div className="inspection-title">
        <strong>{binding.binding_type} · {binding.binding_key}</strong>
        <span>{binding.target_kind} · {binding.status}</span>
      </div>
      <div className="field-grid">
        <KeyValue label="Binding ID" value={shortId(binding.id)} />
        <KeyValue label="Target ID" value={shortId(binding.target_id ?? "none")} />
        <KeyValue label="Source path" value={binding.source_path ?? "none"} />
        <KeyValue label="Pack" value={`${binding.pack_id}@${binding.pack_version}`} />
        <KeyValue label="Created" value={binding.created_at} />
        <KeyValue label="Updated" value={binding.updated_at} />
      </div>
      <details className="json-details">
        <summary>Materialized payload</summary>
        <pre>{formatJson(binding.materialized_payload)}</pre>
      </details>
    </div>
  );
}

function SchedulerPanel({ summary }: { summary: SchedulerOrchestrationSummary }) {
  const semanticSchedule = summary.plan.actions.find((item) => item.action === "semantic_synthesis_schedule_run");
  const dueActions = summary.plan.actions.filter((item) => item.due_count > 0);
  return (
    <div className="scheduler-panel">
      <div className="graph-summary">
        <KeyValue label="Status" value={summary.status} />
        <KeyValue label="Plan" value={`${summary.plan.status} · ${summary.plan.actionable_count} due`} />
        <KeyValue label="Last run" value={summary.last_run_status ? `${summary.last_run_status} · ${relativeAge(summary.last_run_at ?? summary.generated_at)}` : "none"} />
      </div>
      {semanticSchedule ? (
        <div className="obs-row">
          <strong>semantic synthesis · {semanticSchedule.status}</strong>
          <span>
            {semanticSchedule.due_count} due · {semanticSchedule.skipped_count} skipped · {semanticSchedule.reason}
          </span>
        </div>
      ) : <p className="muted">No semantic synthesis schedule is registered.</p>}
      {dueActions.slice(0, 4).map((item) => (
        <div key={`${item.area}-${item.action}`} className="obs-row">
          <strong>{item.area} · {item.action}</strong>
          <span>{item.due_count} due · {item.severity} · {item.reason}</span>
        </div>
      ))}
      {summary.recent_runs.slice(0, 3).map((run) => (
        <div key={run.audit_log_id} className="obs-row">
          <strong>{run.status} · {relativeAge(run.created_at)}</strong>
          <span>{run.actions.length ? run.actions.join(", ") : "no scheduler actions"}</span>
        </div>
      ))}
    </div>
  );
}

function GrantRow({ grant }: { grant: TaskGrant }) {
  const scope = grant.connector_scope?.mode;
  return (
    <div className="obs-row">
      <strong>{grant.agent_class ?? "grant"} · {grant.status}</strong>
      <span>{grant.risk_level} · {typeof scope === "string" ? scope : "tool scoped"} · {shortId(grant.id)}</span>
    </div>
  );
}

function MemoryPartitionDetail({ detail }: { detail: MemoryGovernancePartitionDetail }) {
  return (
    <div className="memory-detail">
      <div className="graph-summary">
        <KeyValue label="Access" value={detail.access_policy} />
        <KeyValue label="Objects" value={String(detail.object_count)} />
        <KeyValue label="Pending writes" value={String(detail.pending_writeback_count)} />
      </div>
      {detail.risk_items.slice(0, 3).map((item) => (
        <Row key={`${item.kind}-${item.message}`} title={item.kind} detail={`${item.severity} · ${item.message}`} />
      ))}
      {detail.objects.slice(0, 4).map((object) => (
        <div key={object.id} className="obs-row memory-object-row">
          <strong>{object.title}</strong>
          <span>{object.trust_level} · {object.freshness} · {object.summary}</span>
        </div>
      ))}
    </div>
  );
}

function MemoryWritebackQueuePanel({
  queue,
  fullCandidates,
  status,
  limit,
  selectedId,
  reviewReason,
  isLoadingDetail,
  isReviewing,
  error,
  onStatusChange,
  onLimitChange,
  onSelect,
  onReasonChange,
  onReview,
}: {
  queue: MemoryGovernanceWritebackQueue;
  fullCandidates: MemoryWritebackCandidate[];
  status: string;
  limit: number;
  selectedId: string;
  reviewReason: string;
  isLoadingDetail: boolean;
  isReviewing: boolean;
  error: unknown;
  onStatusChange: (status: string) => void;
  onLimitChange: (limit: number) => void;
  onSelect: (id: string) => void;
  onReasonChange: (reason: string) => void;
  onReview: (id: string, decision: "approve" | "reject", reason?: string) => void;
}) {
  const activeRef = queue.candidates.find((candidate) => candidate.id === selectedId) ?? queue.candidates[0];
  const activeCandidate = activeRef ? fullCandidates.find((candidate) => candidate.id === activeRef.id) : undefined;
  const source = activeCandidate ? sourceLabel(activeCandidate) : "full candidate detail not loaded";
  return (
    <div className="writeback-queue">
      <div className="control-grid">
        <label>
          <span>Status</span>
          <select value={status} onChange={(event) => onStatusChange(event.target.value)}>
            {["pending", "approved", "rejected", "all"].map((value) => (
              <option key={value} value={value}>{value}</option>
            ))}
          </select>
        </label>
        <label>
          <span>Limit</span>
          <input
            type="number"
            min={1}
            max={200}
            value={limit}
            onChange={(event) => onLimitChange(clampNumber(event.target.valueAsNumber, 1, 200, 50))}
          />
        </label>
        <span className="status-hint">
          {queue.candidate_count} shown · {queue.pending_count} pending · {relativeAge(queue.generated_at)}
        </span>
      </div>
      <div className="writeback-workspace">
        <div className="writeback-list">
          {queue.candidates.length ? queue.candidates.map((candidate) => (
            <button
              key={candidate.id}
              className={activeRef?.id === candidate.id ? "writeback-select selected" : "writeback-select"}
              onClick={() => onSelect(candidate.id)}
            >
              <strong>{candidate.title}</strong>
              <span>{candidate.status} · {candidate.partition_key}</span>
              <small>{candidate.summary}</small>
            </button>
          )) : <p className="muted">No writeback candidates match this queue filter.</p>}
        </div>
        {activeRef ? (
          <div className="writeback-inspector">
            <div className="inspection-title">
              <strong>{activeRef.title}</strong>
              <span>{activeRef.status} · {activeRef.candidate_type}</span>
            </div>
            <div className="field-grid">
              <KeyValue label="Source" value={source} />
              <KeyValue label="Partition" value={activeRef.partition_key} />
              <KeyValue label="Session" value={shortId(activeRef.session_id)} />
              <KeyValue label="Proposed key" value={activeRef.proposed_object_key} />
              <KeyValue label="Object type" value={activeRef.proposed_object_type} />
              <KeyValue label="Object ID" value={shortId(activeRef.semantic_object_id ?? "not materialized")} />
              <KeyValue label="Trust" value={activeRef.trust_level} />
              <KeyValue label="Freshness" value={activeRef.freshness} />
              <KeyValue label="Created" value={activeRef.created_at} />
              <KeyValue label="Updated" value={activeRef.updated_at} />
              <KeyValue label="Decided" value={activeRef.decided_at ?? "pending"} />
              <KeyValue label="Candidate ID" value={shortId(activeRef.id)} />
            </div>
            <p className="detail-copy">{activeRef.summary}</p>
            {activeCandidate ? (
              <>
                <div className="field-grid">
                  <KeyValue label="Reviewer" value={activeCandidate.reviewer_subject ?? "none"} />
                  <KeyValue label="Review reason" value={activeCandidate.review_reason ?? "none"} />
                  <KeyValue label="Audit trace" value={shortId(activeCandidate.audit_trace_id ?? "none")} />
                </div>
                <details className="json-details">
                  <summary>Candidate detail</summary>
                  <pre>{formatJson({
                    content: activeCandidate.content,
                    semantic_scopes: activeCandidate.semantic_scopes,
                    source_refs: activeCandidate.source_refs,
                    provenance: activeCandidate.provenance,
                  })}</pre>
                </details>
              </>
            ) : (
              <p className="muted">{isLoadingDetail ? "Loading full candidate detail." : "Full candidate detail was not returned by the global candidate endpoint."}</p>
            )}
            <label className="review-reason">
              <span>Review reason</span>
              <textarea
                rows={3}
                value={reviewReason}
                spellCheck={false}
                onChange={(event) => onReasonChange(event.target.value)}
                placeholder="Optional reason recorded with approve/reject"
              />
            </label>
            <div className="review-actions">
              <button
                disabled={activeRef.status !== "pending" || isReviewing}
                onClick={() => onReview(activeRef.id, "approve", reviewReason)}
              >
                Approve
              </button>
              <button
                disabled={activeRef.status !== "pending" || isReviewing}
                className="ghost danger"
                onClick={() => onReview(activeRef.id, "reject", reviewReason)}
              >
                Reject
              </button>
            </div>
            {error ? <p className="error-note">{errorMessage(error)}</p> : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function MiniCountMap({ title, counts }: { title: string; counts: Record<string, number> }) {
  const entries = Object.entries(counts);
  return (
    <div className="mini-counts">
      <strong>{title}</strong>
      {entries.length ? entries.slice(0, 4).map(([key, value]) => (
        <span key={key}>{key}: {value}</span>
      )) : <span>none</span>}
    </div>
  );
}

function SemanticBackendPanel({ registry }: { registry: SemanticRetrievalBackendRegistry }) {
  return (
    <div className="semantic-backends">
      <div className="graph-summary">
        <KeyValue label="Effective" value={registry.effective_backend} />
        <KeyValue label="Selected" value={registry.selected_backend} />
        <KeyValue label="Fail closed" value={registry.fail_closed ? "yes" : "no"} />
      </div>
      {registry.backends.map((backend) => (
        <div key={backend.backend} className={backend.effective ? "obs-row backend-row selected" : "obs-row backend-row"}>
          <strong>{backend.backend} · {backend.status}</strong>
          <span>
            {backend.backend_type}
            {backend.configured ? " · configured" : ""}
            {backend.missing_env_vars.length ? ` · missing ${backend.missing_env_vars.join(", ")}` : ""}
          </span>
          {backend.blocking_reasons.length ? <small>{backend.blocking_reasons.join("; ")}</small> : null}
        </div>
      ))}
    </div>
  );
}

function ContextPacketTrace({ packet }: { packet: ContextPacket }) {
  return (
    <div className="context-packet">
      <div className="graph-summary">
        <KeyValue label="Packet" value={`v${packet.version} · ${shortId(packet.id)}`} />
        <KeyValue label="Agent" value={packet.agent.name} />
        <KeyValue label="Objects" value={String(packet.retrieved_objects.length)} />
      </div>
      <div className="scope-line">
        {scopePairs(packet.semantic_scopes).map(([key, value]) => (
          <span key={key}>{key}={value}</span>
        ))}
        {!scopePairs(packet.semantic_scopes).length ? <span>no semantic scopes</span> : null}
      </div>
      {packet.freshness_warnings.length ? (
        <div className="warning-list">
          {packet.freshness_warnings.map((warning) => <span key={warning}>{warning}</span>)}
        </div>
      ) : null}
      {packet.retrieved_objects.slice(0, 5).map((object) => (
        <div key={object.id} className="obs-row semantic-object-row">
          <strong>{object.title}</strong>
          <span>{object.object_type} · {object.trust_level} · {object.freshness}</span>
          <small>{object.summary}</small>
        </div>
      ))}
      <details className="json-details">
        <summary>Replay summary</summary>
        <pre>{JSON.stringify(packet.replay_summary, null, 2)}</pre>
      </details>
    </div>
  );
}

function WritebackReviewPanel({
  candidates,
  queue,
  isReviewing,
  onReview,
}: {
  candidates: MemoryWritebackCandidate[];
  queue?: MemoryGovernanceWritebackQueue;
  isReviewing: boolean;
  onReview: (id: string, decision: "approve" | "reject") => void;
}) {
  const visibleCandidates = candidates.length ? candidates : [];
  return (
    <div className="writeback-review">
      {visibleCandidates.length ? visibleCandidates.slice(0, 6).map((candidate) => (
        <div key={candidate.id} className={candidate.status === "pending" ? "review-row pending" : "review-row"}>
          <div>
            <strong>{candidate.title}</strong>
            <span>{candidate.candidate_type} · {candidate.status} · {candidate.trust_level} · {candidate.freshness}</span>
            <small>{candidate.summary}</small>
          </div>
          <div className="review-actions">
            <button disabled={candidate.status !== "pending" || isReviewing} onClick={() => onReview(candidate.id, "approve")}>Approve</button>
            <button disabled={candidate.status !== "pending" || isReviewing} className="ghost danger" onClick={() => onReview(candidate.id, "reject")}>Reject</button>
          </div>
        </div>
      )) : (
        <p className="muted">No session writeback candidates yet. Generate candidates after the session has evidence to preserve.</p>
      )}
      {queue ? (
        <div className="queue-peek">
          <strong>Global queue</strong>
          <span>{queue.candidate_count} shown · {queue.pending_count} pending</span>
        </div>
      ) : null}
    </div>
  );
}

function OntologyRegistryPanel({ registry }: { registry?: OntologyRegistry }) {
  if (!registry) {
    return <p className="muted">Ontology registry is loading.</p>;
  }
  return (
    <div className="ontology-registry">
      <div className="graph-summary">
        <KeyValue label="Version" value={registry.version} />
        <KeyValue label="Objects" value={String(registry.object_types.length)} />
        <KeyValue label="Relations" value={String(registry.relation_types.length)} />
      </div>
      <div className="ontology-matrix">
        {registry.relation_types.map((relation, index) => (
          <div
            key={`${relation.from_entity_type}-${relation.name}-${relation.to_entity_type}-${index}`}
            className="obs-row ontology-row"
          >
            <strong>{relation.from_entity_type} {"->"} {relation.to_entity_type}</strong>
            <span>{relation.name} · {relation.governance_boundary}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function SemanticIngestionPanel({
  draft,
  result,
  isSaving,
  error,
  onDraftChange,
  onCreate,
}: {
  draft: string;
  result?: SemanticIngestionBatchResult;
  isSaving: boolean;
  error: unknown;
  onDraftChange: (value: string) => void;
  onCreate: () => void;
}) {
  return (
    <div className="semantic-ingestion">
      <textarea
        value={draft}
        rows={9}
        spellCheck={false}
        onChange={(event) => onDraftChange(event.target.value)}
      />
      <div className="action-row">
        <button disabled={isSaving || !draft.trim()} onClick={onCreate}>
          {isSaving ? "Ingesting..." : "Ingest batch"}
        </button>
        <span>{result ? `${result.objects.length} objects · ${result.links.length} links` : "waiting"}</span>
      </div>
      {result ? (
        <div className="queue-peek">
          <strong>{result.source.display_name}</strong>
          <span>{result.status} · {shortId(result.source.id)}</span>
        </div>
      ) : null}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
    </div>
  );
}

function SemanticSynthesisPanel({
  sessionId,
  draft,
  result,
  isSaving,
  error,
  onDraftChange,
  onCreate,
}: {
  sessionId: string;
  draft: string;
  result?: SemanticSynthesisRunResult;
  isSaving: boolean;
  error: unknown;
  onDraftChange: (value: string) => void;
  onCreate: () => void;
}) {
  return (
    <div className="semantic-synthesis">
      <textarea
        value={draft}
        rows={9}
        spellCheck={false}
        onChange={(event) => onDraftChange(event.target.value)}
      />
      <div className="action-row">
        <button disabled={isSaving || !sessionId || !draft.trim()} onClick={onCreate}>
          {isSaving ? "Creating..." : "Create synthesis"}
        </button>
        <span>{result ? `${result.artifact.artifact_type} · ${result.candidates.length} candidates` : sessionId ? "waiting" : "select session"}</span>
      </div>
      {result ? (
        <div className="queue-peek">
          <strong>{result.synthesis_type}</strong>
          <span>{shortId(result.artifact.id)} · {relativeAge(result.created_at)}</span>
        </div>
      ) : null}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
    </div>
  );
}

function SemanticObjectBrowser({
  objects,
  objectTypes,
  selectedType,
  onTypeChange,
}: {
  objects: SemanticObject[];
  objectTypes: string[];
  selectedType: string;
  onTypeChange: (value: string) => void;
}) {
  return (
    <div className="semantic-objects">
      <div className="select-row">
        <select value={selectedType} onChange={(event) => onTypeChange(event.target.value)}>
          {objectTypes.map((type) => <option key={type} value={type}>{type}</option>)}
        </select>
        <span>{objects.length} objects</span>
      </div>
      {objects.slice(0, 8).map((object) => (
        <div key={object.id} className="obs-row semantic-object-row">
          <strong>{object.title}</strong>
          <span>{object.object_type} · {object.trust_level} · {object.freshness} · {object.status}</span>
          <small>{object.summary}</small>
          <div className="scope-line compact">
            {scopePairs(object.semantic_scopes).slice(0, 4).map(([key, value]) => (
              <span key={key}>{key}={value}</span>
            ))}
          </div>
        </div>
      ))}
      {!objects.length ? <p className="muted">No semantic objects match this filter.</p> : null}
    </div>
  );
}

function SemanticLinkManager({
  objects,
  links,
  draft,
  registry,
  isSaving,
  error,
  onDraftChange,
  onCreate,
}: {
  objects: SemanticObject[];
  links: SemanticLink[];
  draft: SemanticLinkDraft;
  registry?: OntologyRegistry;
  isSaving: boolean;
  error: unknown;
  onDraftChange: React.Dispatch<React.SetStateAction<SemanticLinkDraft>>;
  onCreate: () => void;
}) {
  const relationOptions = useMemo(() => {
    return (registry?.relation_types ?? []).filter((relation) => (
      relation.from_entity_type === "semantic_object" && relation.to_entity_type === "semantic_object"
    ));
  }, [registry?.relation_types]);
  const selectedRelationIsAllowed = relationOptions.some((relation) => relation.name === draft.relation);
  const canLink = objects.length >= 2 && draft.from && draft.to && draft.from !== draft.to && selectedRelationIsAllowed;
  return (
    <div className="semantic-links">
      <div className="link-editor">
        <select value={draft.from} onChange={(event) => onDraftChange((value) => ({ ...value, from: event.target.value }))}>
          <option value="">From object</option>
          {objects.map((object) => <option key={object.id} value={object.id}>{object.title}</option>)}
        </select>
        <select
          value={draft.relation}
          disabled={!relationOptions.length}
          onChange={(event) => onDraftChange((value) => ({ ...value, relation: event.target.value }))}
        >
          {!relationOptions.length ? <option value="">No allowed relation</option> : null}
          {relationOptions.map((relation) => (
            <option key={relation.name} value={relation.name}>
              {relation.name}
            </option>
          ))}
        </select>
        <select value={draft.to} onChange={(event) => onDraftChange((value) => ({ ...value, to: event.target.value }))}>
          <option value="">To object</option>
          {objects.map((object) => <option key={object.id} value={object.id}>{object.title}</option>)}
        </select>
        <button disabled={!canLink || isSaving} onClick={onCreate}>{isSaving ? "Saving..." : "Link"}</button>
      </div>
      {draft.from && draft.to && draft.from === draft.to ? <p className="error-note">Choose two different semantic objects.</p> : null}
      {draft.relation && relationOptions.length && !selectedRelationIsAllowed ? (
        <p className="error-note">Relation is not allowed by ontology registry.</p>
      ) : null}
      {error ? <p className="error-note">{errorMessage(error)}</p> : null}
      {links.slice(0, 8).map((link) => (
        <div key={link.id} className="obs-row semantic-link-row">
          <strong>{objectTitle(objects, link.from_entity_id)} {"->"} {objectTitle(objects, link.to_entity_id)}</strong>
          <span>{link.relation_type} · {link.status} · confidence {Math.round(link.confidence * 100)}%</span>
        </div>
      ))}
      {!links.length ? <p className="muted">No semantic links recorded yet.</p> : null}
    </div>
  );
}

function runtimeSummary(events: SessionEvent[], agent?: Agent): { provider: string; client: string; execution: string } {
  const latestModelEvent = [...events]
    .reverse()
    .find((event) => event.event_type === "llm.response" || event.event_type === "llm.request");
  const payload = latestModelEvent?.payload as { provider?: unknown; client?: unknown } | undefined;
  const provider = typeof payload?.provider === "string" ? payload.provider : agent?.provider ?? "unknown";
  const client = typeof payload?.client === "string" ? payload.client : "not observed";
  const hasCodex = events.some((event) => event.event_type.startsWith("codex."));
  const hasAgentCli = events.some((event) => event.event_type.startsWith("agent_cli."));
  const hasInternalTools = events.some((event) => event.event_type === "tool.result");
  const execution = hasCodex
    ? "Codex runtime"
    : hasAgentCli
      ? "Agent CLI runtime"
      : hasInternalTools
        ? "Internal tool executors"
        : "No execution observed";
  return { provider, client, execution };
}

function preferredAgent(agents: Agent[]): Agent | undefined {
  return (
    agents.find((agent) => agent.release_state === "active" && agent.tools.length > 0) ??
    agents.find((agent) => agent.tools.length > 0) ??
    agents.find((agent) => agent.release_state === "active") ??
    agents[0]
  );
}

function isWorkbenchView(value: string | null): value is WorkbenchView {
  return value === "agents" || value === "board" || value === "manager" || value === "semantic" || value === "packs" || value === "deploy";
}

function viewTitle(view: WorkbenchView): string {
  return {
    agents: "Managed agent observability",
    board: "Collaboration board",
    manager: "Manager agent desk",
    semantic: "Semantic memory governance",
    packs: "Workflow pack console",
    deploy: "Production deployment control",
  }[view];
}

function planGoal(plan: ManagerAgentPlan): string {
  const goal = plan.task_intake.goal;
  if (typeof goal === "string" && goal.trim()) return goal;
  const title = plan.task_intake.title;
  if (typeof title === "string" && title.trim()) return title;
  return `Manager plan ${shortId(plan.id)}`;
}

function planSteps(plan: ManagerAgentPlan): string[] {
  const steps = plan.decomposition.steps;
  if (Array.isArray(steps)) {
    return steps.filter((step): step is string => typeof step === "string" && step.trim().length > 0);
  }
  const tasks = plan.decomposition.tasks;
  if (Array.isArray(tasks)) {
    return tasks
      .map((task) => {
        if (typeof task === "string") return task;
        if (task && typeof task === "object" && "title" in task && typeof task.title === "string") return task.title;
        return "";
      })
      .filter(Boolean);
  }
  return [];
}

function workItemTitle(items: WorkItem[], itemId?: string | null): string {
  if (!itemId) return "none";
  return items.find((item) => item.id === itemId)?.title ?? shortId(itemId);
}

function readinessStatus(record?: Record<string, unknown>): string {
  if (!record) return "loading";
  return recordStatus(record);
}

function recordStatus(record?: Record<string, unknown>): string {
  if (!record) return "loading";
  for (const key of ["status", "state", "readiness", "health"]) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) return value;
  }
  const nested = record.deployment_readiness;
  if (nested && typeof nested === "object" && "status" in nested && typeof nested.status === "string") {
    return nested.status;
  }
  return "observed";
}

function emptySchedulerSummary(): SchedulerOrchestrationSummary {
  return {
    generated_at: new Date().toISOString(),
    status: "loading",
    plan: {
      status: "loading",
      generated_at: new Date().toISOString(),
      team_count: 0,
      item_count: 0,
      actionable_count: 0,
      actions: [],
    },
    deployment_readiness: {},
    recent_run_count: 0,
    last_run_at: null,
    last_run_status: null,
    last_run_action_count: 0,
    recent_runs: [],
    attention_items: [],
  };
}

function titleFromTask(task: string): string {
  const firstLine = task.trim().split(/\n/)[0] || "Managed agent task";
  return firstLine.length > 64 ? `${firstLine.slice(0, 61)}...` : firstLine;
}

function timestamp(job: WorkerJob): string {
  return job.completed_at ?? job.started_at ?? job.enqueued_at ?? "";
}

function relativeAge(value: string): string {
  const date = Date.parse(value);
  if (Number.isNaN(date)) return "";
  const seconds = Math.max(0, Math.floor((Date.now() - date) / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}

function relativeMoment(value: string): string {
  const date = Date.parse(value);
  if (Number.isNaN(date)) return "";
  const deltaSeconds = Math.floor((date - Date.now()) / 1000);
  const absoluteSeconds = Math.abs(deltaSeconds);
  const suffix = deltaSeconds >= 0 ? "from now" : "ago";
  if (absoluteSeconds < 60) return `${absoluteSeconds}s ${suffix}`;
  const minutes = Math.floor(absoluteSeconds / 60);
  if (minutes < 60) return `${minutes}m ${suffix}`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${suffix}`;
  return `${Math.floor(hours / 24)}d ${suffix}`;
}

function hasPhase(events: SessionEvent[], transitions: WorkflowTransition[], phase: string): boolean {
  if (phase === "workflow") return transitions.length > 0 || hasEvent(events, "workflow");
  return hasEvent(events, phase);
}

function hasEvent(events: SessionEvent[], phase: string): boolean {
  return events.some((event) => event.event_type.startsWith(phase) || event.event_type.includes(phase));
}

function statusFromText(status: string): RowStatus {
  if (status === "completed" || status === "skipped" || status === "consumed") return "completed";
  if (status === "running" || status === "active") return "running";
  if (status === "queued" || status === "scheduled" || status === "issued" || status === "materialized" || status === "deferred" || status === "pending" || status === "planned" || status === "ready") return "queued";
  if (status === "requires_action" || status === "requires_review" || status === "pending_approval" || status === "blocked" || status === "waiting_for_approval") return "needs_input";
  if (status === "failed" || status === "canceled" || status === "denied") return "failed";
  return "idle";
}

function taskBoardColumnFor(item: TaskBoardItemView): TaskBoardColumnId {
  const status = item.status.toLowerCase();
  if (isTerminalTaskStatus(status)) return "done";
  if (item.claimable) return "ready";
  if (isReviewTaskStatus(status)) return "review";
  if (status === "running" || status === "active" || status === "in_progress" || Boolean(item.claimed_by_worker)) return "running";
  if (item.blockers.length || status === "blocked") return "blocked";
  return "backlog";
}

function isTerminalTaskStatus(status: string): boolean {
  return ["completed", "failed", "canceled", "cancelled", "skipped", "denied"].includes(status);
}

function isReviewTaskStatus(status: string): boolean {
  return ["requires_action", "requires_review", "pending_approval", "waiting_for_approval", "review"].includes(status);
}

function compareTaskBoardItems(left: TaskBoardItemView, right: TaskBoardItemView): number {
  if (left.claimable !== right.claimable) return left.claimable ? -1 : 1;
  const priorityDelta = priorityRank(left.work_item_priority) - priorityRank(right.work_item_priority);
  if (priorityDelta !== 0) return priorityDelta;
  return right.updated_at.localeCompare(left.updated_at);
}

function priorityRank(priority?: string | null): number {
  const normalized = (priority ?? "").toLowerCase();
  if (normalized === "urgent" || normalized === "critical" || normalized === "p0") return 0;
  if (normalized === "high" || normalized === "p1") return 1;
  if (normalized === "normal" || normalized === "medium" || normalized === "p2") return 2;
  if (normalized === "low" || normalized === "p3") return 3;
  return 4;
}

function matchesTaskBoardQuery(item: TaskBoardItemView, agents: Agent[], query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const haystack = [
    item.work_item_title,
    item.work_item_priority,
    item.workflow_run_id,
    item.workflow_definition_id,
    item.workflow_step_run_id,
    item.step_key,
    item.step_type,
    item.status,
    item.task_grant_id,
    item.context_packet_id,
    item.claimed_by_worker,
    agentName(agents, item.agent_id),
    ...item.blockers,
  ].filter(Boolean).join(" ").toLowerCase();
  return haystack.includes(normalized);
}

function taskBoardRunBlocker(item: TaskBoardItemView, selectedAgent: Agent | undefined, agents: Agent[]): string {
  if (!item.claimable) return item.blockers[0] ?? "step is not claimable";
  if (!selectedAgent) return "select an agent";
  if (item.agent_id && item.agent_id !== selectedAgent.id) return `select assigned agent ${agentName(agents, item.agent_id)}`;
  return "";
}

function uniqueTransitionTypes(transitions: Array<{ transition_type: string }>): string[] {
  return [...new Set(transitions.map((transition) => transition.transition_type))].sort();
}

function agentName(agents: Agent[], agentId?: string | null): string {
  if (!agentId) return "unassigned";
  return agents.find((agent) => agent.id === agentId)?.name ?? shortId(agentId);
}

function shortId(value: string): string {
  if (!value || value === "none") return value;
  return value.length > 12 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}

function clampNumber(value: number, min: number, max: number, fallback: number): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

function sourceLabel(candidate: MemoryWritebackCandidate): string {
  const sources = [
    candidate.source_event_id ? `event ${shortId(candidate.source_event_id)}` : "",
    candidate.source_artifact_id ? `artifact ${shortId(candidate.source_artifact_id)}` : "",
    candidate.source_approval_id ? `approval ${shortId(candidate.source_approval_id)}` : "",
    candidate.source_handoff_id ? `handoff ${shortId(candidate.source_handoff_id)}` : "",
  ].filter(Boolean);
  return sources.length ? sources.join(" · ") : candidate.candidate_type;
}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2) ?? "";
  } catch {
    return String(value);
  }
}

function summaryLine(value: Record<string, unknown>): string {
  const error = typeof value.error === "string" ? value.error : "";
  const result = typeof value.result === "string" ? value.result : "";
  const skipReason = typeof value.skip_reason === "string" ? value.skip_reason : "";
  const keys = Array.isArray(value.keys) ? value.keys.filter((item) => typeof item === "string").slice(0, 3).join(", ") : "";
  return error || result || skipReason || keys;
}

function scopePairs(scopes: Record<string, unknown>): Array<[string, string]> {
  return Object.entries(scopes)
    .filter(([, value]) => value !== null && value !== undefined && value !== "")
    .slice(0, 6)
    .map(([key, value]) => [key, typeof value === "string" || typeof value === "number" || typeof value === "boolean" ? String(value) : JSON.stringify(value)]);
}

function objectTitle(objects: SemanticObject[], id: string): string {
  return objects.find((object) => object.id === id)?.title ?? shortId(id);
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function eventKind(eventType: string): string {
  if (eventType.includes("approval")) return "approval";
  if (eventType.includes("tool") || eventType.includes("policy")) return "tool";
  if (eventType.includes("llm") || eventType.includes("agent")) return "agent";
  if (eventType.includes("failed") || eventType.includes("error")) return "failed";
  if (eventType.includes("artifact")) return "artifact";
  return "system";
}

function invalidateAll(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries();
}

function firstQueryError(errors: Array<unknown>): string | undefined {
  const error = errors.find(Boolean);
  if (!error) return undefined;
  if (error instanceof Error) return error.message;
  return String(error);
}

function consumeTokenFromHash(): string {
  const hash = window.location.hash.startsWith("#") ? window.location.hash.slice(1) : window.location.hash;
  const params = new URLSearchParams(hash);
  const view = params.get("view");
  if (isWorkbenchView(view)) {
    localStorage.setItem("mandoforge.activeView", view);
  }
  const token = params.get("admin_token") ?? params.get("token") ?? "";
  if (!token.trim()) return "";
  setAdminToken(token);
  window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
  return token;
}
