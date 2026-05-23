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

export type WorkflowStepRun = {
  id: string;
  workflow_run_id: string;
  step_key: string;
  step_type: string;
  agent_id?: string | null;
  session_id?: string | null;
  task_grant_id?: string | null;
  status: string;
  started_at?: string | null;
  completed_at?: string | null;
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
  created_at: string;
  updated_at: string;
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
