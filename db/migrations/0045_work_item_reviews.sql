CREATE TABLE IF NOT EXISTS work_item_reviews (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    work_item_id UUID NOT NULL REFERENCES work_items(id),
    reviewer_kind TEXT NOT NULL,
    reviewer_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'requested',
    decision TEXT,
    summary TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'work_item_reviews'

CREATE INDEX IF NOT EXISTS idx_work_item_reviews_tenant_work_item
    ON work_item_reviews (tenant_id, work_item_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_work_item_reviews_tenant_reviewer
    ON work_item_reviews (tenant_id, reviewer_kind, reviewer_id, status);

ALTER TABLE work_item_reviews ENABLE ROW LEVEL SECURITY;
ALTER TABLE work_item_reviews FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_work_item_reviews ON work_item_reviews;
CREATE POLICY tenant_isolation_work_item_reviews ON work_item_reviews
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
