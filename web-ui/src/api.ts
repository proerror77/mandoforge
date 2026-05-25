export type Agent = {
  id: string;
  name: string;
  kind: string;
  agent_role: string;
  model: string;
  provider: string;
  runtime_profile_id?: string | null;
  tools: string[];
  release_state: string;
};

export type SemanticSource = {
  id: string;
  source_type: string;
  source_uri: string;
  display_name: string;
  owner_type?: string | null;
  owner_id?: string | null;
  metadata: Record<string, unknown>;
  provenance: Record<string, unknown>;
  freshness: Record<string, unknown>;
  status: string;
  last_ingested_at?: string | null;
  created_at: string;
  updated_at: string;
  archived_at?: string | null;
};

export type SemanticObject = {
  id: string;
  source_id?: string | null;
  object_type: string;
  object_key: string;
  title: string;
  summary: string;
  content: Record<string, unknown>;
  semantic_scopes: Record<string, unknown>;
  source_uri?: string | null;
  provenance: Record<string, unknown>;
  trust_level: string;
  freshness: string;
  status: string;
  created_at: string;
  updated_at: string;
  archived_at?: string | null;
};

export type SemanticLink = {
  id: string;
  from_entity_type: string;
  from_entity_id: string;
  relation_type: string;
  to_entity_type: string;
  to_entity_id: string;
  metadata: Record<string, unknown>;
  provenance: Record<string, unknown>;
  confidence: number;
  status: string;
  created_at: string;
  updated_at: string;
  archived_at?: string | null;
};

export type OntologyRegistry = {
  version: string;
  object_types: OntologyObjectType[];
  relation_types: OntologyRelationType[];
};

export type OntologyObjectType = {
  name: string;
  description: string;
  entity_type?: string;
  memory_level?: string;
  governance_boundary: string;
};

export type OntologyRelationType = {
  name: string;
  from_entity_type: string;
  to_entity_type: string;
  description: string;
  governance_boundary: string;
};

export type SemanticIngestionBatchResult = {
  status: string;
  source: SemanticSource;
  objects: SemanticObject[];
  object_refs: Array<{
    temp_ref: string;
    semantic_object_id: string;
    object_key: string;
    title: string;
  }>;
  links: SemanticLink[];
  ingested_at: string;
};

export type SemanticSynthesisRunResult = {
  status: string;
  synthesis_type: string;
  session_id: string;
  checkpoint_event_id: string;
  artifact: Artifact;
  candidates: MemoryWritebackCandidate[];
  created_at: string;
};

export type ContextPacket = {
  id: string;
  session_id: string;
  agent_id: string;
  agent_version_id?: string | null;
  version: number;
  generated_at: string;
  task: Record<string, unknown>;
  agent: {
    id: string;
    name: string;
    kind: string;
    agent_role: string;
    release_state: string;
    tools: string[];
    mcp_server_ids: string[];
    skill_ids: string[];
    workflow_pack_ids: string[];
    remote_computer_profile: Record<string, unknown>;
  };
  runtime_profile?: {
    id: string;
    name: string;
    runtime_type: string;
    remote_computer_required: boolean;
    status: string;
  } | null;
  semantic_scopes: Record<string, unknown>;
  tool_policy: Record<string, unknown>;
  policy_reminders: string[];
  freshness_warnings: string[];
  source_refs: Array<{
    source_type: string;
    source_id: string;
    freshness: string;
  }>;
  retrieved_objects: Array<{
    id: string;
    object_type: string;
    object_key: string;
    title: string;
    summary: string;
    source_id?: string | null;
    source_uri?: string | null;
    trust_level: string;
    freshness: string;
    semantic_scopes: Record<string, unknown>;
    provenance: Record<string, unknown>;
  }>;
  replay_summary: Record<string, unknown>;
  audit_trace_id?: string | null;
  created_at: string;
};

export type MemoryWritebackCandidate = {
  id: string;
  session_id: string;
  candidate_type: string;
  source_event_id?: string | null;
  source_artifact_id?: string | null;
  source_approval_id?: string | null;
  source_handoff_id?: string | null;
  proposed_object_type: string;
  proposed_object_key: string;
  title: string;
  summary: string;
  content: Record<string, unknown>;
  semantic_scopes: Record<string, unknown>;
  source_refs: unknown;
  provenance: Record<string, unknown>;
  trust_level: string;
  freshness: string;
  status: string;
  reviewer_subject?: string | null;
  review_reason?: string | null;
  semantic_object_id?: string | null;
  audit_trace_id?: string | null;
  created_at: string;
  updated_at: string;
  decided_at?: string | null;
};

export type SemanticRetrievalBackendRegistry = {
  selected_backend: string;
  effective_backend: string;
  fail_closed: boolean;
  object_model_required: boolean;
  backends: Array<{
    backend: string;
    backend_type: string;
    status: string;
    selected: boolean;
    effective: boolean;
    configured: boolean;
    required_env_vars: string[];
    missing_env_vars: string[];
    object_link_context_packet_required: boolean;
    blocking_reasons: string[];
  }>;
};

export type Environment = {
  id: string;
  name: string;
  environment_type: string;
  status: string;
  release_state: string;
};

export type Session = {
  id: string;
  agent_id: string;
  agent_version_id: string;
  environment_id?: string | null;
  title: string;
  status: string;
  created_at: string;
  updated_at: string;
};

export type SessionEvent = {
  id?: string;
  seq: number;
  event_type: string;
  payload: Record<string, unknown>;
  created_at: string;
};

export type Approval = {
  id: string;
  session_id: string;
  action: string;
  risk_level: string;
  reason: string;
  evidence: Record<string, unknown>;
  status: string;
  expires_at?: string | null;
};

export type ToolCall = {
  id: string;
  session_id: string;
  tool_name: string;
  status: string;
  risk_level: string;
  policy_decision: Record<string, unknown>;
  result?: unknown;
  error?: unknown;
  created_at: string;
};

export type Artifact = {
  id: string;
  name: string;
  artifact_type: string;
  path?: string | null;
  content?: unknown;
  created_at: string;
};

export type WorkerJob = {
  id: string;
  session_id: string;
  status: string;
  reason?: string;
  tool_name?: string;
  worker_id?: string | null;
  last_error?: string | null;
  enqueued_at?: string;
  started_at?: string | null;
  completed_at?: string | null;
};

export type WorkflowRun = {
  id: string;
  workflow_definition_id: string;
  pack_installation_id?: string | null;
  status: string;
  primary_session_id: string;
  root_task_grant_id?: string | null;
  input_digest: string;
  started_at?: string | null;
  completed_at?: string | null;
  created_at: string;
  updated_at: string;
};

export type WorkflowDefinition = {
  id: string;
  pack_installation_id?: string | null;
  pack_id?: string | null;
  pack_version?: string | null;
  name: string;
  entrypoint: string;
  trigger_type: string;
  default_agent_id: string;
  default_environment_id?: string | null;
  input_schema_ref?: string | null;
  output_schema_ref?: string | null;
  step_graph: Record<string, unknown>;
  handoff_rules: Record<string, unknown>;
  approval_policy_ref?: string | null;
  eval_gate_refs: string[];
  release_state: string;
  created_at: string;
  updated_at: string;
  archived_at?: string | null;
};

export type UpdateWorkflowDefinitionRequest = {
  name?: string;
  entrypoint?: string;
  trigger_type?: string;
  default_agent_id?: string;
  default_environment_id?: string | null;
  input_schema_ref?: string | null;
  output_schema_ref?: string | null;
  step_graph?: Record<string, unknown>;
  handoff_rules?: Record<string, unknown>;
  approval_policy_ref?: string | null;
  eval_gate_refs?: string[];
  release_state?: string;
};

export type WorkflowStepRun = {
  id: string;
  workflow_run_id: string;
  step_key: string;
  step_type: string;
  agent_id?: string | null;
  session_id?: string | null;
  task_grant_id?: string | null;
  status: string;
  input_payload?: Record<string, unknown>;
  output_payload?: Record<string, unknown>;
  claimed_by_worker?: string | null;
  lease_expires_at?: string | null;
  context_packet_id?: string | null;
  started_at?: string | null;
  completed_at?: string | null;
  scheduled_at?: string | null;
  created_at: string;
  updated_at: string;
};

export type WorkflowTransition = {
  id: string;
  workflow_run_id: string;
  from_step_run_id?: string | null;
  from_step_key?: string | null;
  to_step_run_id?: string | null;
  to_step_key?: string | null;
  transition_type: string;
  status: string;
  condition_payload: Record<string, unknown>;
  result_payload: Record<string, unknown>;
  created_at: string;
};

export type WorkflowGraphConsole = {
  workflow_run_id: string;
  workflow_definition_id: string;
  pack_installation_id?: string | null;
  generated_at: string;
  status: string;
  node_count: number;
  edge_count: number;
  due_scheduled_count: number;
  status_counts: Record<string, number>;
  nodes: WorkflowGraphConsoleNode[];
  edges: WorkflowGraphConsoleEdge[];
};

export type WorkflowGraphConsoleNode = {
  id: string;
  step_run_id?: string | null;
  step_key: string;
  step_type: string;
  status: string;
  declared: boolean;
  dependencies: string[];
  agent_id?: string | null;
  task_grant_id?: string | null;
  context_packet_id?: string | null;
  claimed_by_worker?: string | null;
  lease_expires_at?: string | null;
  scheduled_at?: string | null;
  due: boolean;
  started_at?: string | null;
  completed_at?: string | null;
  definition_summary: Record<string, unknown>;
  input_summary: Record<string, unknown>;
  output_summary: Record<string, unknown>;
};

export type WorkflowGraphConsoleEdge = {
  id: string;
  from_step_key?: string | null;
  to_step_key?: string | null;
  transition_type: string;
  status: string;
  declared: boolean;
  condition_summary: Record<string, unknown>;
  result_summary: Record<string, unknown>;
  created_at: string;
};

export type TaskGrant = {
  id: string;
  workflow_run_id: string;
  workflow_step_run_id?: string | null;
  session_id?: string | null;
  grantee_agent_id?: string | null;
  grantee_session_id?: string | null;
  agent_class?: string | null;
  risk_level: string;
  status: string;
  tool_scope: Record<string, unknown>;
  connector_scope: Record<string, unknown>;
  external_effects: Record<string, unknown>;
  context_packet_id?: string | null;
  created_at: string;
  updated_at: string;
};

export type WorkItem = {
  id: string;
  title: string;
  description?: string | null;
  source: string;
  status: string;
  priority: string;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type TaskBoardSnapshot = {
  generated_at: string;
  work_item_count: number;
  workflow_run_count: number;
  workflow_step_count: number;
  claimable_count: number;
  status_counts: Record<string, number>;
  items: TaskBoardItem[];
};

export type TaskBoardItem = {
  work_item_id?: string | null;
  work_item_title?: string | null;
  work_item_priority?: string | null;
  workflow_run_id: string;
  workflow_definition_id: string;
  workflow_step_run_id: string;
  step_key: string;
  step_type: string;
  agent_id?: string | null;
  task_grant_id?: string | null;
  context_packet_id?: string | null;
  status: string;
  claimable: boolean;
  blockers: string[];
  claimed_by_worker?: string | null;
  lease_expires_at?: string | null;
  updated_at: string;
};

export type AgentInboxSnapshot = {
  agent_id: string;
  generated_at: string;
  entry_count: number;
  claimable_count: number;
  entries: AgentInboxEntry[];
};

export type AgentInboxEntry = {
  workflow_run_id: string;
  workflow_definition_id: string;
  workflow_step_run_id: string;
  step_key: string;
  step_type: string;
  status: string;
  task_grant_id?: string | null;
  context_packet_id?: string | null;
  work_item?: WorkItem | null;
  claimable: boolean;
  blockers: string[];
  claimed_by_worker?: string | null;
  lease_expires_at?: string | null;
  input_summary: Record<string, unknown>;
  updated_at: string;
};

export type ClaimWorkflowStepRunResponse = {
  step: WorkflowStepRun;
  task_grant: TaskGrant;
  context_packet: {
    id: string;
    version: number;
    retrieved_objects: Array<Record<string, unknown>>;
    source_refs: Array<Record<string, unknown>>;
  };
};

export type RunWorkflowStepRunResponse = ClaimWorkflowStepRunResponse & {
  session: Session;
  session_loop_job: WorkerJob;
};

export type WorkflowPackBinding = {
  id: string;
  installation_id: string;
  pack_id: string;
  pack_version: string;
  binding_type: string;
  binding_key: string;
  source_path?: string | null;
  target_kind: string;
  target_id?: string | null;
  status: string;
  materialized_payload: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type WorkflowPackRuntimeObject = {
  id: string;
  installation_id: string;
  binding_id: string;
  pack_id: string;
  pack_version: string;
  object_type: string;
  object_key: string;
  runtime_kind: string;
  status: string;
  spec: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type MemoryGovernanceSummary = {
  status: string;
  generated_at: string;
  isolation_policy: string;
  semantic_object_count: number;
  memory_object_count: number;
  partition_count: number;
  partitions: MemoryGovernancePartition[];
  trust_counts: Record<string, number>;
  freshness_counts: Record<string, number>;
  writeback: {
    pending_count: number;
    approved_count: number;
    rejected_count: number;
  };
  attention_items: MemoryGovernanceAttentionItem[];
};

export type MemoryGovernancePartition = {
  partition_key: string;
  domain_scope: string;
  workflow_scope: string;
  memory_scope: string;
  object_count: number;
  memory_object_count: number;
  human_verified_count: number;
  unverified_count: number;
  stale_count: number;
  shared: boolean;
};

export type MemoryGovernanceAttentionItem = {
  severity: string;
  kind: string;
  message: string;
  partition_key?: string | null;
};

export type MemoryGovernancePartitionDetail = {
  generated_at: string;
  partition: MemoryGovernancePartition;
  object_count: number;
  pending_writeback_count: number;
  access_policy: string;
  risk_items: MemoryGovernanceAttentionItem[];
  objects: MemoryGovernanceObjectRef[];
  writeback_candidates: MemoryGovernanceWritebackRef[];
};

export type MemoryGovernanceObjectRef = {
  id: string;
  object_key: string;
  title: string;
  summary: string;
  trust_level: string;
  freshness: string;
  status: string;
  source_uri?: string | null;
  semantic_scopes: Record<string, unknown>;
  provenance: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type MemoryGovernanceWritebackQueue = {
  generated_at: string;
  status_filter?: string | null;
  candidate_count: number;
  pending_count: number;
  candidates: MemoryGovernanceWritebackRef[];
};

export type MemoryGovernanceWritebackRef = {
  id: string;
  session_id: string;
  candidate_type: string;
  proposed_object_type: string;
  proposed_object_key: string;
  title: string;
  summary: string;
  trust_level: string;
  freshness: string;
  status: string;
  partition_key: string;
  semantic_object_id?: string | null;
  created_at: string;
  updated_at: string;
  decided_at?: string | null;
};

export type SchedulerDuePlanItem = {
  area: string;
  action: string;
  mode: string;
  status: string;
  due_count: number;
  skipped_count: number;
  target_count: number;
  severity: string;
  reason: string;
};

export type SchedulerDuePlan = {
  status: string;
  generated_at: string;
  team_count: number;
  item_count: number;
  actionable_count: number;
  actions: SchedulerDuePlanItem[];
};

export type SchedulerRunHistoryItem = {
  audit_log_id: string;
  run_id?: string | null;
  idempotency_key?: string | null;
  owner?: string | null;
  status: string;
  team_count: number;
  action_count: number;
  actions: string[];
  created_at: string;
};

export type SchedulerOrchestrationSummary = {
  generated_at: string;
  status: string;
  plan: SchedulerDuePlan;
  deployment_readiness: Record<string, unknown>;
  recent_run_count: number;
  last_run_at?: string | null;
  last_run_status?: string | null;
  last_run_action_count: number;
  recent_runs: SchedulerRunHistoryItem[];
  attention_items: Array<{
    severity: string;
    kind: string;
    message: string;
  }>;
};

const ADMIN_TOKEN_KEY = "mandoforge.adminToken";

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      "content-type": "application/json",
      ...authHeaders(),
      ...(init.headers ?? {}),
    },
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return response.json() as Promise<T>;
}

export function approvalHeaders(): HeadersInit {
  return adminHeaders();
}

export function adminHeaders(): HeadersInit {
  return authHeaders();
}

export function getAdminToken(): string {
  return localStorage.getItem(ADMIN_TOKEN_KEY) ?? "";
}

export function setAdminToken(token: string): void {
  const normalized = token.trim();
  if (normalized) {
    localStorage.setItem(ADMIN_TOKEN_KEY, normalized);
  } else {
    localStorage.removeItem(ADMIN_TOKEN_KEY);
  }
}

export function authHeaders(): HeadersInit {
  const token = getAdminToken();
  return {
    ...(token ? { authorization: `Bearer ${token}` } : {}),
    "x-mandoforge-subject": localStorage.getItem("mandoforge.approverSubject") || "admin-1",
    "x-mandoforge-roles": localStorage.getItem("mandoforge.approverRoles") || "admin",
  };
}

export async function createSession(input: {
  agent_id: string;
  environment_id?: string;
  title: string;
}): Promise<Session> {
  return api<Session>("/api/sessions", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function createAgent(input: {
  name: string;
  kind?: string;
  provider?: string;
  model?: string;
  agent_role?: string;
  system_prompt?: string;
  tools?: string[];
  tool_policy?: Record<string, unknown>;
  mcp_server_ids?: string[];
  skill_ids?: string[];
  workflow_pack_ids?: string[];
  semantic_scopes?: Record<string, unknown>;
  release_state?: string;
}): Promise<Agent> {
  return api<Agent>("/api/agents", {
    method: "POST",
    headers: adminHeaders(),
    body: JSON.stringify(input),
  });
}

export async function appendUserMessage(sessionId: string, message: string): Promise<unknown> {
  return api(`/api/sessions/${sessionId}/events`, {
    method: "POST",
    body: JSON.stringify({
      events: [{ type: "user.message", payload: { message } }],
    }),
  });
}

export async function decideApproval(id: string, decision: "approve" | "reject"): Promise<Approval> {
  return api<Approval>(`/api/approvals/${id}/${decision}`, {
    method: "POST",
    headers: approvalHeaders(),
  });
}

export async function reviewMemoryWritebackCandidate(
  id: string,
  decision: "approve" | "reject",
  reason?: string,
): Promise<MemoryWritebackCandidate> {
  const normalizedReason = reason?.trim();
  return api<MemoryWritebackCandidate>(`/api/memory-writeback-candidates/${id}/${decision}`, {
    method: "POST",
    headers: approvalHeaders(),
    body: JSON.stringify(normalizedReason ? { reason: normalizedReason } : {}),
  });
}

export async function updateWorkflowDefinition(
  id: string,
  input: UpdateWorkflowDefinitionRequest,
): Promise<WorkflowDefinition> {
  return api<WorkflowDefinition>(`/api/workflow-definitions/${id}`, {
    method: "PATCH",
    headers: adminHeaders(),
    body: JSON.stringify(input),
  });
}
