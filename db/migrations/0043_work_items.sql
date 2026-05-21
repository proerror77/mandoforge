CREATE TABLE IF NOT EXISTS work_items (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    organization_id UUID REFERENCES organizations(id),
    team_id UUID REFERENCES teams(id),
    project_id UUID REFERENCES projects(id),
    title TEXT NOT NULL,
    description TEXT,
    source TEXT NOT NULL,
    source_url TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    priority TEXT NOT NULL DEFAULT 'normal',
    assignee TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'work_items'

CREATE INDEX IF NOT EXISTS idx_work_items_tenant_status_created
    ON work_items (tenant_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_work_items_tenant_scope
    ON work_items (tenant_id, organization_id, team_id, project_id);

ALTER TABLE work_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE work_items FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_work_items ON work_items;
CREATE POLICY tenant_isolation_work_items ON work_items
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
