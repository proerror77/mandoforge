CREATE TABLE IF NOT EXISTS remote_computers (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    profile TEXT NOT NULL,
    status TEXT NOT NULL,
    namespace TEXT NOT NULL,
    pod_name TEXT,
    workspace_path TEXT NOT NULL,
    state_mount_path TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_remote_computers_tenant_status
    ON remote_computers (tenant_id, status);

CREATE TABLE IF NOT EXISTS remote_computer_leases (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    remote_computer_id UUID NOT NULL REFERENCES remote_computers(id),
    session_id UUID REFERENCES sessions(id),
    status TEXT NOT NULL,
    worker_id TEXT,
    lease_expires_at TIMESTAMPTZ,
    heartbeat_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_remote_computer_leases_tenant_status
    ON remote_computer_leases (tenant_id, status);

CREATE INDEX IF NOT EXISTS idx_remote_computer_leases_session
    ON remote_computer_leases (tenant_id, session_id);
