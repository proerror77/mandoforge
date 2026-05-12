CREATE TABLE IF NOT EXISTS codex_app_server_runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    operation TEXT NOT NULL,
    thread_id TEXT,
    turn_id TEXT,
    command_id TEXT,
    status TEXT NOT NULL,
    request JSONB NOT NULL DEFAULT '{}',
    response JSONB NOT NULL DEFAULT '{}',
    error JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_codex_app_server_runs_tenant_created
    ON codex_app_server_runs(tenant_id, created_at DESC);

