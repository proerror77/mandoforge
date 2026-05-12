CREATE TABLE IF NOT EXISTS agent_releases (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    agent_version_id UUID NOT NULL REFERENCES agent_versions(id),
    environment TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'promoted',
    eval_run_id UUID REFERENCES eval_runs(id),
    eval_score DOUBLE PRECISION,
    min_score DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    promoted_by TEXT,
    promoted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_agent_releases_agent ON agent_releases(tenant_id, agent_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_releases_environment ON agent_releases(tenant_id, environment, created_at DESC);
