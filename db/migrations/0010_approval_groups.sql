CREATE TABLE IF NOT EXISTS approval_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    subjects JSONB NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

CREATE TABLE IF NOT EXISTS approval_escalation_rules (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    risk_level TEXT NOT NULL,
    group_id UUID NOT NULL REFERENCES approval_groups(id),
    order_index INT NOT NULL DEFAULT 0,
    after_seconds INT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

CREATE INDEX IF NOT EXISTS idx_approval_groups_status ON approval_groups(tenant_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_approval_escalation_rules_risk ON approval_escalation_rules(tenant_id, risk_level, status, order_index);
