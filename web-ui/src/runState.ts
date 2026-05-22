import type { Agent, Approval, Artifact, Environment, Session, SessionEvent, ToolCall, WorkerJob } from "./api";

export type RunStatus = "queued" | "running" | "needs_input" | "completed" | "failed" | "idle";

export type RunPhase = {
  label: string;
  detail: string;
  state: "done" | "active" | "waiting" | "skipped" | "failed";
};

export type RunViewModel = {
  status: RunStatus;
  statusLabel: string;
  message: string;
  agent?: Agent;
  environment?: Environment;
  pendingApprovals: Approval[];
  latestJob?: WorkerJob;
  workerJobs: WorkerJob[];
  events: SessionEvent[];
  toolCalls: ToolCall[];
  artifacts: Artifact[];
  phases: RunPhase[];
};

type BuildRunStateInput = {
  session?: Session;
  agents: Agent[];
  environments: Environment[];
  events: SessionEvent[];
  approvals: Approval[];
  executionJobs: WorkerJob[];
  sessionLoopJobs: WorkerJob[];
  toolCalls: ToolCall[];
  artifacts: Artifact[];
};

export function buildRunState(input: BuildRunStateInput): RunViewModel {
  const { session, events, toolCalls, artifacts } = input;
  const agent = input.agents.find((candidate) => candidate.id === session?.agent_id);
  const environment = input.environments.find((candidate) => candidate.id === session?.environment_id);
  const pendingApprovals = input.approvals.filter(
    (approval) => approval.status === "pending" && approval.session_id === session?.id,
  );
  const workerJobs = [...input.sessionLoopJobs, ...input.executionJobs]
    .filter((job) => job.session_id === session?.id)
    .sort((left, right) => timestamp(right).localeCompare(timestamp(left)));
  const completedEventJob = session ? completedLoopJobFromEvent(events, session.id) : undefined;
  const latestJob = finalized(events)
    ? workerJobs.find((job) => job.status === "completed") ?? completedEventJob ?? workerJobs[0]
    : workerJobs[0];
  const failedJob = workerJobs.find((job) => job.status === "failed");
  const status = deriveStatus(session, events, pendingApprovals, failedJob);
  const message = runMessage(status, pendingApprovals, failedJob, latestJob, events);

  return {
    status,
    statusLabel: labelForStatus(status),
    message,
    agent,
    environment,
    pendingApprovals,
    latestJob,
    workerJobs,
    events,
    toolCalls,
    artifacts,
    phases: phases(events, pendingApprovals, failedJob),
  };
}

function deriveStatus(
  session: Session | undefined,
  events: SessionEvent[],
  pendingApprovals: Approval[],
  failedJob: WorkerJob | undefined,
): RunStatus {
  if (failedJob || session?.status === "failed") return "failed";
  if (pendingApprovals.length > 0 || session?.status === "requires_action") return "needs_input";
  if (finalized(events)) return "completed";
  if (session?.status === "running") return "running";
  if (events.some((event) => event.event_type === "session.loop.queued")) return "queued";
  return "idle";
}

function labelForStatus(status: RunStatus): string {
  return {
    queued: "Queued",
    running: "Working",
    needs_input: "Needs input",
    completed: "Completed",
    failed: "Failed",
    idle: "Idle",
  }[status];
}

function runMessage(
  status: RunStatus,
  approvals: Approval[],
  failedJob: WorkerJob | undefined,
  latestJob: WorkerJob | undefined,
  events: SessionEvent[],
): string {
  if (failedJob?.last_error) return failedJob.last_error;
  if (approvals[0]) return `${approvals[0].action} requires approval before the run can continue.`;
  if (status === "completed") return "Agent finished. Timeline, tool calls, and artifacts are available below.";
  if (status === "running") return latestJob ? `Worker ${latestJob.worker_id ?? "pending"} is processing ${latestJob.tool_name ?? latestJob.reason ?? "the run"}.` : "Worker is processing the run.";
  if (status === "queued") return "Run accepted. Waiting for a worker to start.";
  const latest = events.at(-1);
  return latest ? `Latest event #${latest.seq}: ${latest.event_type}` : "Start a task to create a run.";
}

function phases(events: SessionEvent[], approvals: Approval[], failedJob?: WorkerJob): RunPhase[] {
  const has = (types: string[]) => events.some((event) => types.includes(event.event_type));
  const isFinal = finalized(events);
  return [
    { label: "Queued", detail: "Task accepted into the session loop", state: has(["session.loop.queued"]) ? "done" : "waiting" },
    { label: "Planning", detail: "Agent produced a plan and tool calls", state: has(["agent.plan", "llm.response"]) ? "done" : "waiting" },
    { label: "Tools", detail: "Policy evaluated and tools executed", state: has(["tool.result", "policy.allowed"]) ? "done" : "waiting" },
    {
      label: "Approval",
      detail: approvals[0] ? `${approvals[0].action} is waiting for approval` : "No approval required for this run",
      state: approvals[0] ? "active" : has(["approval.requested"]) ? "done" : "skipped",
    },
    {
      label: "Finish",
      detail: failedJob?.last_error ?? "Final message and artifacts are ready",
      state: failedJob ? "failed" : isFinal ? "done" : "waiting",
    },
  ];
}

function finalized(events: SessionEvent[]): boolean {
  return events.some((event) => event.event_type === "agent.final" || Boolean((event.payload as { final_report?: unknown }).final_report));
}

function timestamp(job: WorkerJob): string {
  return job.completed_at ?? job.started_at ?? job.enqueued_at ?? "";
}

function completedLoopJobFromEvent(events: SessionEvent[], sessionId: string): WorkerJob | undefined {
  const event = [...events].reverse().find((candidate) => candidate.event_type === "session.loop.completed");
  if (!event) return undefined;
  const payload = event.payload as {
    session_loop_job_id?: string;
    status?: string;
    worker_id?: string;
  };
  return {
    id: payload.session_loop_job_id ?? `${sessionId}-completed-loop`,
    session_id: sessionId,
    status: payload.status ?? "completed",
    reason: "session loop",
    worker_id: payload.worker_id ?? null,
    completed_at: event.created_at,
  };
}
