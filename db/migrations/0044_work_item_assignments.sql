CREATE TABLE IF NOT EXISTS work_item_assignments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    work_item_id UUID NOT NULL REFERENCES work_items(id),
    assignee_kind TEXT NOT NULL,
    assignee_id TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'owner',
    status TEXT NOT NULL DEFAULT 'assigned',
    assigned_by TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'work_item_assignments'

CREATE INDEX IF NOT EXISTS idx_work_item_assignments_tenant_work_item
    ON work_item_assignments (tenant_id, work_item_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_work_item_assignments_tenant_assignee
    ON work_item_assignments (tenant_id, assignee_kind, assignee_id, status);

ALTER TABLE work_item_assignments ENABLE ROW LEVEL SECURITY;
ALTER TABLE work_item_assignments FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_work_item_assignments ON work_item_assignments;
CREATE POLICY tenant_isolation_work_item_assignments ON work_item_assignments
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
