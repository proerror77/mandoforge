CREATE TABLE IF NOT EXISTS remote_computer_session_attachments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    remote_computer_id UUID NOT NULL REFERENCES remote_computers(id),
    lease_id UUID NOT NULL REFERENCES remote_computer_leases(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    status TEXT NOT NULL,
    attached_by TEXT,
    stale_after TIMESTAMPTZ,
    released_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_remote_computer_attachments_tenant_status
    ON remote_computer_session_attachments (tenant_id, status);

CREATE INDEX IF NOT EXISTS idx_remote_computer_attachments_session
    ON remote_computer_session_attachments (tenant_id, session_id);

CREATE INDEX IF NOT EXISTS idx_remote_computer_attachments_stale
    ON remote_computer_session_attachments (tenant_id, status, stale_after);
