CREATE TABLE IF NOT EXISTS secret_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    key TEXT NOT NULL,
    scope_type TEXT NOT NULL DEFAULT 'tenant',
    scope_id UUID,
    status TEXT NOT NULL DEFAULT 'active',
    version INT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

CREATE INDEX IF NOT EXISTS idx_secret_records_scope ON secret_records(tenant_id, scope_type, scope_id);
