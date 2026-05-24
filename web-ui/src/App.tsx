import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  appendUserMessage,
  api,
  createSession,
  decideApproval,
  getAdminToken,
  setAdminToken,
  type AgentInboxSnapshot,
  type Agent,
  type Approval,
  type Artifact,
  type ContextPacket,
  type Environment,
  type MemoryGovernancePartitionDetail,
  type MemoryGovernanceSummary,
  type MemoryGovernanceWritebackQueue,
  type MemoryWritebackCandidate,
  type OntologyRegistry,
  type RunWorkflowStepRunResponse,
  type SchedulerOrchestrationSummary,
  type SemanticIngestionBatchResult,
  type SemanticLink,
  type SemanticObject,
  type SemanticRetrievalBackendRegistry,
  type SemanticSynthesisRunResult,
  type Session,
  type SessionEvent,
  type TaskGrant,
  type TaskBoardSnapshot,
  type ToolCall,
  type WorkerJob,
  type WorkflowGraphConsole,
  type WorkflowPackBinding,
  type WorkflowPackRuntimeObject,
  type WorkflowRun,
  type WorkflowStepRun,
  type WorkflowTransition,
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

export function App() {
  const queryClient = useQueryClient();
  const [selectedSessionId, setSelectedSessionId] = useState(() => localStorage.getItem("mandoforge.activeSessionId") ?? "");
  const [selectedAgentId, setSelectedAgentId] = useState("");
  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState("");
  const [task, setTask] = useState(DEFAULT_TASK);
  const [adminTokenInput, setAdminTokenInput] = useState(() => consumeTokenFromHash() || getAdminToken());
  const [transitionFilter, setTransitionFilter] = useState("all");
  const [memoryPartitionKey, setMemoryPartitionKey] = useState("");
  const [writebackStatus, setWritebackStatus] = useState("pending");
  const [selectedContextPacketId, setSelectedContextPacketId] = useState("");
  const [semanticObjectType, setSemanticObjectType] = useState("all");
  const [semanticIngestionDraft, setSemanticIngestionDraft] = useState(DEFAULT_INGESTION_BATCH);
  const [semanticSynthesisDraft, setSemanticSynthesisDraft] = useState(DEFAULT_SYNTHESIS_RUN);
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
  const taskBoard = useQuery({ queryKey: ["task-board"], queryFn: () => api<TaskBoardSnapshot>("/api/task-board"), refetchInterval: 1500 });

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
  const packInstallationId = selectedWorkflowRun?.pack_installation_id ?? "";
  const selectedAgent = agents.data?.find((agent) => agent.id === selectedSession?.agent_id) ?? preferredAgent(agents.data ?? []);
  const selectedEnvironment = environments.data?.find((environment) => environment.id === selectedSession?.environment_id) ?? environments.data?.[0];
  const inboxAgentId = selectedAgent?.id ?? "";
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
    params.set("limit", "50");
    if (transitionFilter !== "all") params.set("transition_type", transitionFilter);
    return params.toString();
  }, [transitionFilter]);
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
  const memoryWritebacks = useQuery({
    queryKey: ["memory-governance-writebacks", writebackStatus],
    queryFn: () => api<MemoryGovernanceWritebackQueue>(`/api/memory-governance/writebacks?status=${encodeURIComponent(writebackStatus)}`),
    refetchInterval: 5000,
  });
  const schedulerSummary = useQuery({
    queryKey: ["scheduler-summary"],
    queryFn: () => api<SchedulerOrchestrationSummary>("/api/scheduler/summary"),
    refetchInterval: 5000,
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
    mutationFn: ({ id, decision }: { id: string; decision: "approve" | "reject" }) => api<MemoryWritebackCandidate>(`/api/memory-writeback-candidates/${id}/${decision}`, {
      method: "POST",
      body: JSON.stringify({ reason: `reviewed from semantic console: ${decision}` }),
    }),
    onSuccess: () => invalidateAll(queryClient),
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
    agentInbox.error,
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
          <h1>Managed agent observability</h1>
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
          <Panel title="Task board">
            {taskBoard.data ? (
              <TaskBoardPanel board={taskBoard.data} agents={agents.data ?? []} />
            ) : <p className="muted">Task board is loading.</p>}
          </Panel>

          <Panel title="Agent inbox">
            {agentInbox.data ? (
              <AgentInboxPanel
                inbox={agentInbox.data}
                agent={selectedAgent}
                isRunning={runStep.isPending}
                onRun={(stepId) => {
                  if (selectedAgent) runStep.mutate({ stepId, agentId: selectedAgent.id });
                }}
              />
            ) : <p className="muted">No agent inbox loaded.</p>}
          </Panel>

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

          <Panel title="Steps">
            {(workflowSteps.data ?? []).length ? (workflowSteps.data ?? []).map((step) => (
              <StepRow key={step.id} step={step} />
            )) : <p className="muted">No workflow steps observed.</p>}
          </Panel>

          <Panel title="Transitions">
            {transitionTypes.length ? (
              <div className="filter-row">
                {["all", ...transitionTypes].map((type) => (
                  <button
                    key={type}
                    className={transitionFilter === type ? "filter-chip selected" : "filter-chip"}
                    onClick={() => setTransitionFilter(type)}
                  >
                    {type}
                  </button>
                ))}
              </div>
            ) : null}
            {filteredTransitions.length ? filteredTransitions.slice(-10).reverse().map((transition) => (
              <TransitionRow key={transition.id} transition={transition} />
            )) : <p className="muted">No durable transition records yet.</p>}
          </Panel>

          <Panel title="Pack runtime">
            {packInstallationId ? (
              <>
                {(workflowPackRuntimeObjects.data ?? []).length ? (workflowPackRuntimeObjects.data ?? []).map((object) => (
                  <RuntimeObjectRow key={object.id} object={object} />
                )) : <p className="muted">No runtime objects materialized.</p>}
                {(workflowPackBindings.data ?? []).slice(0, 5).map((binding) => (
                  <BindingRow key={binding.id} binding={binding} />
                ))}
              </>
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
                <div className="filter-row">
                  {["pending", "approved", "rejected"].map((status) => (
                    <button
                      key={status}
                      className={writebackStatus === status ? "filter-chip selected" : "filter-chip"}
                      onClick={() => setWritebackStatus(status)}
                    >
                      {status}
                    </button>
                  ))}
                </div>
                {memoryWritebacks.data ? (
                  <MemoryWritebackQueuePanel queue={memoryWritebacks.data} />
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

function TaskBoardPanel({ board, agents }: { board: TaskBoardSnapshot; agents: Agent[] }) {
  return (
    <div className="task-board-panel">
      <div className="graph-summary">
        <KeyValue label="Work items" value={String(board.work_item_count)} />
        <KeyValue label="Steps" value={String(board.workflow_step_count)} />
        <KeyValue label="Claimable" value={String(board.claimable_count)} />
      </div>
      <div className="status-counts">
        {Object.entries(board.status_counts).map(([status, count]) => (
          <span key={status} className={`status-count node-${statusFromText(status)}`}>{status}: {count}</span>
        ))}
      </div>
      {board.items.slice(0, 6).map((item) => (
        <div key={item.workflow_step_run_id} className={item.claimable ? "obs-row task-board-row claimable" : "obs-row task-board-row"}>
          <StatusLogo status={statusFromText(item.status)} />
          <div>
            <strong>{item.work_item_title ?? item.step_key}</strong>
            <span>
              {item.step_key} · {item.status} · {agentName(agents, item.agent_id)}
              {item.context_packet_id ? ` · ctx ${shortId(item.context_packet_id)}` : ""}
            </span>
            {item.blockers.length ? <small>{item.blockers.join(", ")}</small> : <small>ready for worker run</small>}
          </div>
        </div>
      ))}
    </div>
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
      {inbox.entries.slice(0, 6).map((entry) => (
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

function RuntimeObjectRow({ object }: { object: WorkflowPackRuntimeObject }) {
  const providerValidation = object.spec?.provider_specific_validation;
  return (
    <div className="obs-row runtime-row">
      <strong>{object.object_type} · {object.status}</strong>
      <span>{object.object_key} · {typeof providerValidation === "string" ? providerValidation : object.runtime_kind}</span>
    </div>
  );
}

function BindingRow({ binding }: { binding: WorkflowPackBinding }) {
  return (
    <div className="obs-row binding-row">
      <strong>{binding.binding_type} · {binding.binding_key}</strong>
      <span>{binding.target_kind} · {binding.status}</span>
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

function MemoryWritebackQueuePanel({ queue }: { queue: MemoryGovernanceWritebackQueue }) {
  return (
    <div className="writeback-queue">
      <KeyValue label="Writeback queue" value={`${queue.candidate_count} shown · ${queue.pending_count} pending`} />
      {queue.candidates.slice(0, 4).map((candidate) => (
        <div key={candidate.id} className="obs-row writeback-row">
          <strong>{candidate.title}</strong>
          <span>{candidate.status} · {candidate.partition_key} · {candidate.summary}</span>
        </div>
      ))}
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
  if (status === "queued" || status === "scheduled" || status === "issued" || status === "materialized" || status === "deferred") return "queued";
  if (status === "failed" || status === "canceled" || status === "denied") return "failed";
  return "idle";
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
  const token = params.get("admin_token") ?? params.get("token") ?? "";
  if (!token.trim()) return "";
  setAdminToken(token);
  window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
  return token;
}
