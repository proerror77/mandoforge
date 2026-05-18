CREATE TABLE IF NOT EXISTS context_packets (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    agent_version_id UUID,
    version BIGINT NOT NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    task JSONB NOT NULL DEFAULT '{}',
    agent JSONB NOT NULL DEFAULT '{}',
    runtime_profile JSONB,
    semantic_scopes JSONB NOT NULL DEFAULT '{}',
    tool_policy JSONB NOT NULL DEFAULT '{}',
    policy_reminders JSONB NOT NULL DEFAULT '[]',
    freshness_warnings JSONB NOT NULL DEFAULT '[]',
    source_refs JSONB NOT NULL DEFAULT '[]',
    retrieved_objects JSONB NOT NULL DEFAULT '[]',
    replay_summary JSONB NOT NULL DEFAULT '{}',
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'context_packets'

CREATE UNIQUE INDEX IF NOT EXISTS idx_context_packets_session_version
    ON context_packets (tenant_id, session_id, version);

CREATE INDEX IF NOT EXISTS idx_context_packets_session_created
    ON context_packets (tenant_id, session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_context_packets_agent_created
    ON context_packets (tenant_id, agent_id, created_at DESC);

ALTER TABLE context_packets ENABLE ROW LEVEL SECURITY;
ALTER TABLE context_packets FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_context_packets ON context_packets;
CREATE POLICY tenant_isolation_context_packets ON context_packets
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
