CREATE TABLE IF NOT EXISTS agent_handoff_assignments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    agent_handoff_event_id UUID NOT NULL REFERENCES agent_handoff_events(id),
    manager_plan_id UUID NOT NULL REFERENCES manager_agent_plans(id),
    source_session_id UUID NOT NULL REFERENCES sessions(id),
    specialist_session_id UUID NOT NULL REFERENCES sessions(id),
    source_agent_id UUID NOT NULL REFERENCES agents(id),
    target_agent_id UUID NOT NULL REFERENCES agents(id),
    semantic_scopes JSONB NOT NULL DEFAULT '{}',
    runtime_profile_id UUID REFERENCES agent_runtime_profiles(id),
    remote_computer_required BOOLEAN NOT NULL DEFAULT false,
    remote_computer_job_assignment_id UUID REFERENCES remote_computer_job_assignments(id),
    status TEXT NOT NULL,
    assigned_by TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'agent_handoff_assignments'

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_handoff_assignments_tenant_handoff
    ON agent_handoff_assignments (tenant_id, agent_handoff_event_id);

CREATE INDEX IF NOT EXISTS idx_agent_handoff_assignments_tenant_manager_plan
    ON agent_handoff_assignments (tenant_id, manager_plan_id);

CREATE INDEX IF NOT EXISTS idx_agent_handoff_assignments_tenant_sessions
    ON agent_handoff_assignments (tenant_id, source_session_id, specialist_session_id);

CREATE INDEX IF NOT EXISTS idx_agent_handoff_assignments_tenant_remote_status
    ON agent_handoff_assignments (tenant_id, remote_computer_required, status);

ALTER TABLE agent_handoff_assignments ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_handoff_assignments FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_agent_handoff_assignments ON agent_handoff_assignments;
CREATE POLICY tenant_isolation_agent_handoff_assignments ON agent_handoff_assignments
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
