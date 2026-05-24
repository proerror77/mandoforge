import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  appendUserMessage,
  api,
  createSession,
  decideApproval,
  getAdminToken,
  setAdminToken,
  type Agent,
  type Approval,
  type Artifact,
  type Environment,
  type MemoryGovernancePartitionDetail,
  type MemoryGovernanceSummary,
  type MemoryGovernanceWritebackQueue,
  type Session,
  type SessionEvent,
  type TaskGrant,
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

  const agents = useQuery({ queryKey: ["agents"], queryFn: () => api<Agent[]>("/api/agents"), refetchInterval: 5000 });
  const environments = useQuery({ queryKey: ["environments"], queryFn: () => api<Environment[]>("/api/environments"), refetchInterval: 5000 });
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: () => api<Session[]>("/api/sessions"), refetchInterval: 1800 });
  const approvals = useQuery({ queryKey: ["approvals"], queryFn: () => api<Approval[]>("/api/approvals"), refetchInterval: 1800 });
  const executionJobs = useQuery({ queryKey: ["execution-jobs"], queryFn: () => api<WorkerJob[]>("/api/execution-jobs"), refetchInterval: 1500 });
  const sessionLoopJobs = useQuery({ queryKey: ["session-loop-jobs"], queryFn: () => api<WorkerJob[]>("/api/session-loop-jobs"), refetchInterval: 1500 });
  const allToolCalls = useQuery({ queryKey: ["tool-calls"], queryFn: () => api<ToolCall[]>("/api/tool-calls"), refetchInterval: 1800 });
  const workflowRuns = useQuery({ queryKey: ["workflow-runs"], queryFn: () => api<WorkflowRun[]>("/api/workflow-runs"), refetchInterval: 1600 });

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
  const workflowTransitions = useQuery({
    queryKey: ["workflow-transitions", workflowRunId],
    queryFn: () => api<WorkflowTransition[]>(`/api/workflow-runs/${workflowRunId}/transitions`),
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
  const visibleToolCalls = sessionToolCalls.data?.length
    ? sessionToolCalls.data
    : (allToolCalls.data ?? []).filter((call) => call.session_id === sessionId);
  const runtime = runtimeSummary(events.data ?? [], selectedAgent);
  const transitionTypes = uniqueTransitionTypes(workflowTransitions.data ?? []);
  const filteredTransitions = transitionFilter === "all"
    ? (workflowTransitions.data ?? [])
    : (workflowTransitions.data ?? []).filter((transition) => transition.transition_type === transitionFilter);

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

  const activeCount = rows.filter((row) => row.status === "running" || row.status === "queued").length;
  const blockedCount = rows.filter((row) => row.status === "needs_input").length;
  const apiError = firstQueryError([agents.error, environments.error, sessions.error, approvals.error, executionJobs.error, sessionLoopJobs.error, allToolCalls.error]);

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
            {(workflowTransitions.data ?? []).length ? (
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

          <Panel title="Memory governance">
            {memoryGovernance.data ? (
              <>
                <KeyValue label="Status" value={memoryGovernance.data.status} />
                <KeyValue label="Isolation" value={memoryGovernance.data.isolation_policy} />
                <KeyValue label="Memory objects" value={String(memoryGovernance.data.memory_object_count)} />
                <KeyValue label="Partitions" value={String(memoryGovernance.data.partition_count)} />
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
        <span>{step.step_type} · {step.status}{step.scheduled_at ? ` · due ${relativeAge(step.scheduled_at)}` : ""}</span>
      </div>
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
              <span>{node.step_type} · {node.status}{node.scheduled_at ? ` · ${node.due ? "due now" : `due ${relativeAge(node.scheduled_at)}`}` : ""}</span>
              <small>{summaryLine(node.output_summary) || summaryLine(node.input_summary)}</small>
            </div>
          </div>
        ))}
      </div>
      <div className="graph-edges">
        {graph.edges.slice(-8).map((edge) => (
          <span key={edge.id}>
            {(edge.from_step_key ?? "start")} {"->"} {(edge.to_step_key ?? "run")} · {edge.transition_type}/{edge.status}
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

function uniqueTransitionTypes(transitions: WorkflowTransition[]): string[] {
  return [...new Set(transitions.map((transition) => transition.transition_type))].sort();
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
