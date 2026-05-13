CREATE TABLE IF NOT EXISTS approval_notification_channel_policies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    channel TEXT NOT NULL,
    target_env TEXT,
    risk_filter TEXT NOT NULL DEFAULT 'all',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

CREATE INDEX IF NOT EXISTS idx_approval_notification_channel_policies_status
    ON approval_notification_channel_policies(tenant_id, status, created_at DESC);
