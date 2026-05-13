CREATE TABLE IF NOT EXISTS remote_computer_state_locks (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    lock_key TEXT NOT NULL,
    status TEXT NOT NULL,
    remote_computer_id UUID REFERENCES remote_computers(id),
    lease_id UUID REFERENCES remote_computer_leases(id),
    session_id UUID REFERENCES sessions(id),
    owner TEXT,
    expires_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'remote_computer_state_locks'

CREATE INDEX IF NOT EXISTS idx_remote_computer_state_locks_tenant_created
    ON remote_computer_state_locks (tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_remote_computer_state_locks_tenant_session
    ON remote_computer_state_locks (tenant_id, session_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_computer_state_locks_active_key
    ON remote_computer_state_locks (tenant_id, lock_key)
    WHERE status = 'held';

ALTER TABLE remote_computer_state_locks ENABLE ROW LEVEL SECURITY;
ALTER TABLE remote_computer_state_locks FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_remote_computer_state_locks ON remote_computer_state_locks;
CREATE POLICY tenant_isolation_remote_computer_state_locks ON remote_computer_state_locks
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
