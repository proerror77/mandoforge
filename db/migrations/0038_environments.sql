CREATE TABLE IF NOT EXISTS environments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    environment_type TEXT NOT NULL DEFAULT 'local',
    runtime_profile_id UUID REFERENCES agent_runtime_profiles(id),
    remote_computer_profile JSONB NOT NULL DEFAULT '{}',
    codex_app_server_profile JSONB NOT NULL DEFAULT '{}',
    worker_queue_binding JSONB NOT NULL DEFAULT '{}',
    state_mounts JSONB NOT NULL DEFAULT '{}',
    network_policy JSONB NOT NULL DEFAULT '{}',
    vault_requirements JSONB NOT NULL DEFAULT '{}',
    mcp_requirements JSONB NOT NULL DEFAULT '{}',
    release_state TEXT NOT NULL DEFAULT 'draft',
    status TEXT NOT NULL DEFAULT 'enabled',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'environments'

CREATE UNIQUE INDEX IF NOT EXISTS idx_environments_tenant_name_active
    ON environments (tenant_id, lower(name))
    WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_environments_tenant_status
    ON environments (tenant_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_environments_tenant_type
    ON environments (tenant_id, environment_type, created_at DESC);

ALTER TABLE environments ENABLE ROW LEVEL SECURITY;
ALTER TABLE environments FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_environments ON environments;
CREATE POLICY tenant_isolation_environments ON environments
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS environment_id UUID REFERENCES environments(id);

CREATE INDEX IF NOT EXISTS idx_sessions_tenant_environment
    ON sessions (tenant_id, environment_id, created_at DESC)
    WHERE environment_id IS NOT NULL;
