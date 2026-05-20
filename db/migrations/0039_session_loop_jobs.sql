CREATE TABLE IF NOT EXISTS session_loop_jobs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    environment_id UUID REFERENCES environments(id),
    status TEXT NOT NULL DEFAULT 'queued',
    trigger_event_id UUID REFERENCES session_events(id),
    reason TEXT NOT NULL DEFAULT 'user.message',
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    worker_id TEXT,
    lease_expires_at TIMESTAMPTZ,
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 3,
    last_error TEXT
);

-- tracked tenant table: 'session_loop_jobs'

CREATE INDEX IF NOT EXISTS idx_session_loop_jobs_tenant_status
    ON session_loop_jobs (tenant_id, status, enqueued_at ASC);

CREATE INDEX IF NOT EXISTS idx_session_loop_jobs_tenant_session
    ON session_loop_jobs (tenant_id, session_id, enqueued_at DESC);

DROP INDEX IF EXISTS idx_session_loop_jobs_one_active_per_session;

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_loop_jobs_one_queued_per_session
    ON session_loop_jobs (tenant_id, session_id)
    WHERE status = 'queued';

CREATE UNIQUE INDEX IF NOT EXISTS idx_session_loop_jobs_one_running_per_session
    ON session_loop_jobs (tenant_id, session_id)
    WHERE status = 'running';

ALTER TABLE session_loop_jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE session_loop_jobs FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_session_loop_jobs ON session_loop_jobs;
CREATE POLICY tenant_isolation_session_loop_jobs ON session_loop_jobs
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
