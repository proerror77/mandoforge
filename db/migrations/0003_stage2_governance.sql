CREATE TABLE IF NOT EXISTS organizations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ,
    UNIQUE(tenant_id, slug)
);

CREATE TABLE IF NOT EXISTS teams (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ,
    UNIQUE(organization_id, slug)
);

CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    team_id UUID NOT NULL REFERENCES teams(id),
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ,
    UNIQUE(team_id, slug)
);

CREATE TABLE IF NOT EXISTS memberships (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    user_id TEXT NOT NULL,
    organization_id UUID REFERENCES organizations(id),
    team_id UUID REFERENCES teams(id),
    project_id UUID REFERENCES projects(id),
    role TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS provider_access (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    team_id UUID NOT NULL REFERENCES teams(id),
    provider_name TEXT NOT NULL,
    model_allowlist JSONB NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(team_id, provider_name)
);

CREATE TABLE IF NOT EXISTS mcp_servers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    team_id UUID NOT NULL REFERENCES teams(id),
    name TEXT NOT NULL,
    transport TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    tool_allowlist JSONB NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(team_id, name)
);

CREATE TABLE IF NOT EXISTS eval_datasets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS eval_cases (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    dataset_id UUID NOT NULL REFERENCES eval_datasets(id),
    input JSONB NOT NULL,
    expected JSONB,
    grading_policy JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS eval_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    dataset_id UUID NOT NULL REFERENCES eval_datasets(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    agent_version_id UUID NOT NULL REFERENCES agent_versions(id),
    status TEXT NOT NULL,
    score DOUBLE PRECISION,
    details JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE agents ADD COLUMN IF NOT EXISTS team_id UUID REFERENCES teams(id);
ALTER TABLE agents ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES projects(id);
ALTER TABLE memberships ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES projects(id);

CREATE INDEX IF NOT EXISTS idx_teams_organization ON teams(tenant_id, organization_id);
CREATE INDEX IF NOT EXISTS idx_projects_team ON projects(tenant_id, team_id);
CREATE INDEX IF NOT EXISTS idx_memberships_org ON memberships(tenant_id, organization_id);
CREATE INDEX IF NOT EXISTS idx_memberships_team ON memberships(tenant_id, team_id);
CREATE INDEX IF NOT EXISTS idx_memberships_project ON memberships(tenant_id, project_id);
CREATE INDEX IF NOT EXISTS idx_provider_access_team ON provider_access(tenant_id, team_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_tenant_name ON providers(tenant_id, name);
CREATE INDEX IF NOT EXISTS idx_mcp_servers_team ON mcp_servers(tenant_id, team_id);
CREATE INDEX IF NOT EXISTS idx_eval_cases_dataset ON eval_cases(tenant_id, dataset_id);
CREATE INDEX IF NOT EXISTS idx_eval_runs_dataset ON eval_runs(tenant_id, dataset_id);
CREATE INDEX IF NOT EXISTS idx_eval_runs_agent ON eval_runs(tenant_id, agent_id);

ALTER TABLE organizations ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;
ALTER TABLE teams ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;
