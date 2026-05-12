CREATE TABLE IF NOT EXISTS policy_revisions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    body JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    activated_at TIMESTAMPTZ,
    UNIQUE(tenant_id, name)
);

CREATE INDEX IF NOT EXISTS idx_policy_revisions_status ON policy_revisions(tenant_id, status, created_at DESC);
