CREATE TABLE IF NOT EXISTS work_item_activity_entries (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    work_item_id UUID NOT NULL REFERENCES work_items(id),
    event_type TEXT NOT NULL,
    actor_subject TEXT,
    subject_type TEXT,
    subject_id UUID,
    summary TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'work_item_activity_entries'

CREATE INDEX IF NOT EXISTS idx_work_item_activity_entries_tenant_work_item
    ON work_item_activity_entries (tenant_id, work_item_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_work_item_activity_entries_tenant_event_type
    ON work_item_activity_entries (tenant_id, event_type, created_at DESC);

ALTER TABLE work_item_activity_entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE work_item_activity_entries FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_work_item_activity_entries ON work_item_activity_entries;
CREATE POLICY tenant_isolation_work_item_activity_entries ON work_item_activity_entries
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
