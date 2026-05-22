CREATE TABLE IF NOT EXISTS agent_teammates (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    agent_id UUID REFERENCES agents(id),
    display_name TEXT NOT NULL,
    handle TEXT,
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'agent_teammates'

CREATE INDEX IF NOT EXISTS idx_agent_teammates_tenant_agent
    ON agent_teammates (tenant_id, agent_id)
    WHERE agent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_teammates_tenant_handle
    ON agent_teammates (tenant_id, handle)
    WHERE handle IS NOT NULL;

ALTER TABLE agent_teammates ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_teammates FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_agent_teammates ON agent_teammates;
CREATE POLICY tenant_isolation_agent_teammates ON agent_teammates
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

CREATE TABLE IF NOT EXISTS squads (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    purpose TEXT,
    status TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'squads'

CREATE INDEX IF NOT EXISTS idx_squads_tenant_status
    ON squads (tenant_id, status, created_at DESC);

ALTER TABLE squads ENABLE ROW LEVEL SECURITY;
ALTER TABLE squads FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_squads ON squads;
CREATE POLICY tenant_isolation_squads ON squads
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

CREATE TABLE IF NOT EXISTS squad_members (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    squad_id UUID NOT NULL REFERENCES squads(id),
    teammate_id UUID NOT NULL REFERENCES agent_teammates(id),
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'squad_members'

CREATE INDEX IF NOT EXISTS idx_squad_members_tenant_squad
    ON squad_members (tenant_id, squad_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_squad_members_tenant_teammate
    ON squad_members (tenant_id, teammate_id);

ALTER TABLE squad_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE squad_members FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_squad_members ON squad_members;
CREATE POLICY tenant_isolation_squad_members ON squad_members
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
