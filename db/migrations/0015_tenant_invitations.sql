CREATE TABLE IF NOT EXISTS tenant_invitations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    team_id UUID REFERENCES teams(id),
    project_id UUID REFERENCES projects(id),
    email TEXT NOT NULL,
    role TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    token TEXT NOT NULL,
    invited_by TEXT,
    accepted_by TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ,
    UNIQUE(tenant_id, token)
);

CREATE INDEX IF NOT EXISTS idx_tenant_invitations_org ON tenant_invitations(tenant_id, organization_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tenant_invitations_status ON tenant_invitations(tenant_id, status, expires_at);
