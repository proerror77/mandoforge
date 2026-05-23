CREATE TABLE IF NOT EXISTS workflow_transitions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    workflow_run_id UUID NOT NULL REFERENCES workflow_runs(id),
    from_step_run_id UUID REFERENCES workflow_step_runs(id),
    from_step_key TEXT,
    to_step_run_id UUID REFERENCES workflow_step_runs(id),
    to_step_key TEXT,
    transition_type TEXT NOT NULL,
    status TEXT NOT NULL,
    condition_payload JSONB NOT NULL DEFAULT '{}',
    result_payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'workflow_transitions'

CREATE INDEX IF NOT EXISTS idx_workflow_transitions_tenant_run
    ON workflow_transitions (tenant_id, workflow_run_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_workflow_transitions_tenant_type_status
    ON workflow_transitions (tenant_id, transition_type, status, created_at DESC);

ALTER TABLE workflow_transitions ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_transitions FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_transitions ON workflow_transitions;
CREATE POLICY tenant_isolation_workflow_transitions ON workflow_transitions
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
