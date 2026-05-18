CREATE TABLE IF NOT EXISTS agent_runtime_profiles (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    runtime_type TEXT NOT NULL,
    command TEXT NOT NULL,
    default_args JSONB NOT NULL DEFAULT '[]',
    env JSONB NOT NULL DEFAULT '{}',
    timeout_seconds BIGINT,
    remote_computer_required BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL DEFAULT 'enabled',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'agent_runtime_profiles'

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_runtime_profiles_tenant_name_active
    ON agent_runtime_profiles (tenant_id, lower(name))
    WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_agent_runtime_profiles_tenant_status
    ON agent_runtime_profiles (tenant_id, status, created_at DESC);

ALTER TABLE agent_runtime_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_runtime_profiles FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_agent_runtime_profiles ON agent_runtime_profiles;
CREATE POLICY tenant_isolation_agent_runtime_profiles ON agent_runtime_profiles
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
