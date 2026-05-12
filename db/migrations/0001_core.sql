CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS tenants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE DEFAULT 'default',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    current_version INT NOT NULL DEFAULT 1,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    tools JSONB NOT NULL DEFAULT '[]',
    runtime_config JSONB NOT NULL DEFAULT '{}',
    created_by UUID,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS agent_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id),
    version INT NOT NULL,
    model_provider_id UUID,
    model TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    tools JSONB NOT NULL DEFAULT '[]',
    tool_names JSONB NOT NULL DEFAULT '[]',
    runtime_config JSONB NOT NULL DEFAULT '{}',
    approval_policy JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(agent_id, version)
);

CREATE TABLE IF NOT EXISTS providers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    provider_type TEXT NOT NULL,
    name TEXT NOT NULL,
    base_url TEXT,
    encrypted_api_key BYTEA,
    default_model TEXT,
    config JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    agent_version_id UUID REFERENCES agent_versions(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'created',
    created_by UUID,
    runtime_config JSONB NOT NULL DEFAULT '{}',
    error JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS session_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    seq BIGINT NOT NULL,
    parent_event_id UUID,
    actor_type TEXT,
    actor_id UUID,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(session_id, seq)
);

CREATE TABLE IF NOT EXISTS tool_definitions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    description TEXT,
    schema JSONB NOT NULL DEFAULT '{}',
    risk_level TEXT NOT NULL DEFAULT 'low',
    executor_type TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, name)
);

CREATE TABLE IF NOT EXISTS workspaces (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID REFERENCES sessions(id),
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    sandbox_mode TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS artifacts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    workspace_id UUID REFERENCES workspaces(id),
    artifact_type TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT,
    content JSONB,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS tool_calls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    event_id UUID REFERENCES session_events(id),
    tool_name TEXT NOT NULL,
    args JSONB NOT NULL DEFAULT '{}',
    result JSONB,
    status TEXT NOT NULL DEFAULT 'pending',
    risk_level TEXT NOT NULL DEFAULT 'low',
    policy_decision JSONB NOT NULL DEFAULT '{}',
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS approvals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    tool_call_id UUID REFERENCES tool_calls(id),
    action TEXT NOT NULL,
    risk_level TEXT NOT NULL,
    reason TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}',
    requested_reason TEXT,
    requested_payload JSONB NOT NULL DEFAULT '{}',
    decision_payload JSONB NOT NULL DEFAULT '{}',
    approved_by UUID,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID REFERENCES sessions(id),
    actor_type TEXT NOT NULL,
    actor_id UUID,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id UUID,
    details JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS execution_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    approval_id UUID NOT NULL REFERENCES approvals(id),
    tool_call_id UUID NOT NULL REFERENCES tool_calls(id),
    tool_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    enqueued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    worker_id TEXT,
    lease_expires_at TIMESTAMPTZ,
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 3,
    last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_session_events_session_seq ON session_events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_tool_calls_session ON tool_calls(session_id);
CREATE INDEX IF NOT EXISTS idx_approvals_status ON approvals(status);
CREATE INDEX IF NOT EXISTS idx_audit_logs_session ON audit_logs(session_id);
CREATE INDEX IF NOT EXISTS idx_execution_jobs_status ON execution_jobs(tenant_id, status, enqueued_at);

ALTER TABLE tenants ADD COLUMN IF NOT EXISTS slug TEXT NOT NULL DEFAULT 'default';
CREATE UNIQUE INDEX IF NOT EXISTS idx_tenants_slug ON tenants(slug);

ALTER TABLE agents ADD COLUMN IF NOT EXISTS description TEXT;
ALTER TABLE agents ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS current_version INT NOT NULL DEFAULT 1;
ALTER TABLE agents ADD COLUMN IF NOT EXISTS runtime_config JSONB NOT NULL DEFAULT '{}';
ALTER TABLE agents ADD COLUMN IF NOT EXISTS created_by UUID;
ALTER TABLE agents ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE agent_versions ADD COLUMN IF NOT EXISTS model_provider_id UUID;
ALTER TABLE agent_versions ADD COLUMN IF NOT EXISTS tool_names JSONB NOT NULL DEFAULT '[]';
ALTER TABLE agent_versions ADD COLUMN IF NOT EXISTS runtime_config JSONB NOT NULL DEFAULT '{}';

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS created_by UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS agent_version_id UUID REFERENCES agent_versions(id);
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS runtime_config JSONB NOT NULL DEFAULT '{}';
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS error JSONB;

ALTER TABLE artifacts ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}';

ALTER TABLE tool_calls ADD COLUMN IF NOT EXISTS event_id UUID REFERENCES session_events(id);
ALTER TABLE tool_calls ADD COLUMN IF NOT EXISTS risk_level TEXT NOT NULL DEFAULT 'low';

ALTER TABLE approvals ADD COLUMN IF NOT EXISTS tool_call_id UUID REFERENCES tool_calls(id);
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS requested_reason TEXT;
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS requested_payload JSONB NOT NULL DEFAULT '{}';
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS decision_payload JSONB NOT NULL DEFAULT '{}';
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS approved_by UUID;

ALTER TABLE execution_jobs ADD COLUMN IF NOT EXISTS worker_id TEXT;
ALTER TABLE execution_jobs ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;
ALTER TABLE execution_jobs ADD COLUMN IF NOT EXISTS attempt_count INT NOT NULL DEFAULT 0;
ALTER TABLE execution_jobs ADD COLUMN IF NOT EXISTS max_attempts INT NOT NULL DEFAULT 3;
ALTER TABLE execution_jobs ADD COLUMN IF NOT EXISTS last_error TEXT;
