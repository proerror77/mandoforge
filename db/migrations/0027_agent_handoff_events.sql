CREATE TABLE IF NOT EXISTS agent_handoff_events (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    source_session_id UUID NOT NULL REFERENCES sessions(id),
    source_agent_id UUID NOT NULL REFERENCES agents(id),
    target_agent_id UUID NOT NULL REFERENCES agents(id),
    intent TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    schema_version TEXT NOT NULL,
    risk_level TEXT NOT NULL,
    approval_required BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL,
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'agent_handoff_events'

CREATE INDEX IF NOT EXISTS idx_agent_handoff_events_tenant_session_created
    ON agent_handoff_events (tenant_id, source_session_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_agent_handoff_events_tenant_agents
    ON agent_handoff_events (tenant_id, source_agent_id, target_agent_id);

CREATE INDEX IF NOT EXISTS idx_agent_handoff_events_tenant_status
    ON agent_handoff_events (tenant_id, status);

ALTER TABLE agent_handoff_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_handoff_events FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_agent_handoff_events ON agent_handoff_events;
CREATE POLICY tenant_isolation_agent_handoff_events ON agent_handoff_events
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
