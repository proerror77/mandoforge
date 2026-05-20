CREATE TABLE IF NOT EXISTS session_threads (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    parent_thread_id UUID REFERENCES session_threads(id),
    thread_kind TEXT NOT NULL DEFAULT 'primary',
    agent_id UUID NOT NULL REFERENCES agents(id),
    agent_version_id UUID REFERENCES agent_versions(id),
    environment_id UUID REFERENCES environments(id),
    source_handoff_id UUID REFERENCES agent_handoff_events(id),
    specialist_session_id UUID REFERENCES sessions(id),
    status TEXT NOT NULL DEFAULT 'idle',
    title TEXT NOT NULL,
    context JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'session_threads'

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_threads_one_primary
    ON session_threads (tenant_id, session_id)
    WHERE thread_kind = 'primary';

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_threads_one_handoff
    ON session_threads (tenant_id, source_handoff_id)
    WHERE source_handoff_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_session_threads_tenant_session
    ON session_threads (tenant_id, session_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_session_threads_tenant_parent
    ON session_threads (tenant_id, parent_thread_id, created_at ASC)
    WHERE parent_thread_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_session_threads_tenant_status
    ON session_threads (tenant_id, status, updated_at DESC);

ALTER TABLE session_threads ENABLE ROW LEVEL SECURITY;
ALTER TABLE session_threads FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_session_threads ON session_threads;
CREATE POLICY tenant_isolation_session_threads ON session_threads
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
