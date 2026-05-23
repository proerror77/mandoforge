CREATE TABLE IF NOT EXISTS task_grants (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    workflow_run_id UUID NOT NULL REFERENCES workflow_runs(id),
    workflow_step_run_id UUID REFERENCES workflow_step_runs(id),
    session_id UUID REFERENCES sessions(id),
    parent_grant_id UUID REFERENCES task_grants(id),
    source_event_id UUID REFERENCES session_events(id),
    source_handoff_id UUID REFERENCES agent_handoff_events(id),
    issuer_subject TEXT NOT NULL,
    grantee_agent_id UUID REFERENCES agents(id),
    grantee_session_id UUID REFERENCES sessions(id),
    agent_class TEXT,
    objective TEXT NOT NULL,
    risk_level TEXT NOT NULL DEFAULT 'low',
    status TEXT NOT NULL DEFAULT 'active',
    expires_at TIMESTAMPTZ,
    max_turns INTEGER,
    max_tool_calls INTEGER,
    max_runtime_seconds INTEGER,
    max_cost_usd_micros BIGINT,
    semantic_scopes JSONB NOT NULL DEFAULT '{}',
    memory_scope JSONB NOT NULL DEFAULT '{}',
    tool_scope JSONB NOT NULL DEFAULT '{}',
    connector_scope JSONB NOT NULL DEFAULT '{}',
    approval_policy JSONB NOT NULL DEFAULT '{}',
    external_effects JSONB NOT NULL DEFAULT '{}',
    context_packet_id UUID REFERENCES context_packets(id),
    policy_revision_id UUID REFERENCES policy_revisions(id),
    immutable_args_hash TEXT,
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'task_grants'

CREATE INDEX IF NOT EXISTS idx_task_grants_tenant_workflow_run
    ON task_grants (tenant_id, workflow_run_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_task_grants_tenant_parent
    ON task_grants (tenant_id, parent_grant_id)
    WHERE parent_grant_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_task_grants_tenant_status
    ON task_grants (tenant_id, status, updated_at DESC);

ALTER TABLE task_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE task_grants FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_task_grants ON task_grants;
CREATE POLICY tenant_isolation_task_grants ON task_grants
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_workflow_runs_root_task_grant'
    ) THEN
        ALTER TABLE workflow_runs
            ADD CONSTRAINT fk_workflow_runs_root_task_grant
            FOREIGN KEY (root_task_grant_id) REFERENCES task_grants(id);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_workflow_step_runs_task_grant'
    ) THEN
        ALTER TABLE workflow_step_runs
            ADD CONSTRAINT fk_workflow_step_runs_task_grant
            FOREIGN KEY (task_grant_id) REFERENCES task_grants(id);
    END IF;
END $$;
