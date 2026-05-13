CREATE TABLE IF NOT EXISTS remote_computer_sidecar_heartbeats (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    remote_computer_id UUID NOT NULL REFERENCES remote_computers(id),
    session_id UUID REFERENCES sessions(id),
    assignment_id UUID,
    sidecar_name TEXT NOT NULL,
    status TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'remote_computer_sidecar_heartbeats'

CREATE INDEX IF NOT EXISTS idx_remote_computer_sidecar_heartbeats_tenant_observed
    ON remote_computer_sidecar_heartbeats (tenant_id, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_remote_computer_sidecar_heartbeats_tenant_remote
    ON remote_computer_sidecar_heartbeats (tenant_id, remote_computer_id, observed_at DESC);

ALTER TABLE remote_computer_sidecar_heartbeats ENABLE ROW LEVEL SECURITY;
ALTER TABLE remote_computer_sidecar_heartbeats FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_remote_computer_sidecar_heartbeats ON remote_computer_sidecar_heartbeats;
CREATE POLICY tenant_isolation_remote_computer_sidecar_heartbeats ON remote_computer_sidecar_heartbeats
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
