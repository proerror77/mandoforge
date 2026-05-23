CREATE TABLE IF NOT EXISTS workflow_definitions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    pack_installation_id UUID REFERENCES workflow_pack_installations(id),
    pack_id TEXT,
    pack_version TEXT,
    name TEXT NOT NULL,
    entrypoint TEXT NOT NULL,
    trigger_type TEXT NOT NULL DEFAULT 'manual',
    default_agent_id UUID NOT NULL REFERENCES agents(id),
    default_environment_id UUID REFERENCES environments(id),
    input_schema_ref TEXT,
    output_schema_ref TEXT,
    step_graph JSONB NOT NULL DEFAULT '{}',
    handoff_rules JSONB NOT NULL DEFAULT '{}',
    approval_policy_ref TEXT,
    eval_gate_refs JSONB NOT NULL DEFAULT '[]',
    release_state TEXT NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

-- tracked tenant table: 'workflow_definitions'

CREATE INDEX IF NOT EXISTS idx_workflow_definitions_tenant_pack
    ON workflow_definitions (tenant_id, pack_installation_id, release_state);

CREATE INDEX IF NOT EXISTS idx_workflow_definitions_tenant_entrypoint
    ON workflow_definitions (tenant_id, entrypoint, created_at DESC);

ALTER TABLE workflow_definitions ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_definitions FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_definitions ON workflow_definitions;
CREATE POLICY tenant_isolation_workflow_definitions ON workflow_definitions
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

CREATE TABLE IF NOT EXISTS workflow_runs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    workflow_definition_id UUID NOT NULL REFERENCES workflow_definitions(id),
    pack_installation_id UUID REFERENCES workflow_pack_installations(id),
    source_event_id UUID REFERENCES session_events(id),
    source_work_item_id UUID REFERENCES work_items(id),
    source_schedule_id UUID,
    status TEXT NOT NULL DEFAULT 'queued',
    primary_session_id UUID NOT NULL REFERENCES sessions(id),
    root_task_grant_id UUID,
    input_payload JSONB NOT NULL DEFAULT '{}',
    input_digest TEXT NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'workflow_runs'

CREATE INDEX IF NOT EXISTS idx_workflow_runs_tenant_definition
    ON workflow_runs (tenant_id, workflow_definition_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_tenant_status
    ON workflow_runs (tenant_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_tenant_session
    ON workflow_runs (tenant_id, primary_session_id);

ALTER TABLE workflow_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_runs FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_runs ON workflow_runs;
CREATE POLICY tenant_isolation_workflow_runs ON workflow_runs
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

CREATE TABLE IF NOT EXISTS workflow_step_runs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    workflow_run_id UUID NOT NULL REFERENCES workflow_runs(id),
    step_key TEXT NOT NULL,
    step_type TEXT NOT NULL,
    agent_id UUID REFERENCES agents(id),
    agent_version_id UUID REFERENCES agent_versions(id),
    session_id UUID REFERENCES sessions(id),
    thread_id UUID REFERENCES session_threads(id),
    handoff_id UUID REFERENCES agent_handoff_events(id),
    task_grant_id UUID,
    environment_id UUID REFERENCES environments(id),
    status TEXT NOT NULL DEFAULT 'queued',
    input_payload JSONB NOT NULL DEFAULT '{}',
    output_payload JSONB NOT NULL DEFAULT '{}',
    artifact_ids JSONB NOT NULL DEFAULT '[]',
    approval_ids JSONB NOT NULL DEFAULT '[]',
    tool_call_ids JSONB NOT NULL DEFAULT '[]',
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'workflow_step_runs'

CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_tenant_run
    ON workflow_step_runs (tenant_id, workflow_run_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_tenant_status
    ON workflow_step_runs (tenant_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_tenant_session
    ON workflow_step_runs (tenant_id, session_id)
    WHERE session_id IS NOT NULL;

ALTER TABLE workflow_step_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_step_runs FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_step_runs ON workflow_step_runs;
CREATE POLICY tenant_isolation_workflow_step_runs ON workflow_step_runs
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
