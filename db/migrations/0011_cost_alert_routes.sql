CREATE TABLE IF NOT EXISTS cost_alert_routes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    channel TEXT NOT NULL,
    target TEXT,
    severity_filter TEXT NOT NULL DEFAULT 'warning',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

CREATE INDEX IF NOT EXISTS idx_cost_alert_routes_status ON cost_alert_routes(tenant_id, status, created_at DESC);
