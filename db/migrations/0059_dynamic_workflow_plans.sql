CREATE TABLE IF NOT EXISTS dynamic_workflow_plans (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    source_work_item_id UUID REFERENCES work_items(id),
    source_session_id UUID REFERENCES sessions(id),
    objective TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'proposed',
    phases JSONB NOT NULL DEFAULT '[]',
    agent_fleet_policy JSONB NOT NULL DEFAULT '{}',
    governance JSONB NOT NULL DEFAULT '{}',
    validation JSONB NOT NULL DEFAULT '{}',
    materialization JSONB NOT NULL DEFAULT '{}',
    analysis JSONB NOT NULL DEFAULT '{}',
    review JSONB NOT NULL DEFAULT '{}',
    workflow_definition_id UUID REFERENCES workflow_definitions(id),
    workflow_run_id UUID REFERENCES workflow_runs(id),
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at TIMESTAMPTZ,
    materialized_at TIMESTAMPTZ
);

-- tracked tenant table: 'dynamic_workflow_plans'

CREATE INDEX IF NOT EXISTS idx_dynamic_workflow_plans_tenant_status
    ON dynamic_workflow_plans (tenant_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_dynamic_workflow_plans_tenant_work_item
    ON dynamic_workflow_plans (tenant_id, source_work_item_id, created_at DESC)
    WHERE source_work_item_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_dynamic_workflow_plans_tenant_session
    ON dynamic_workflow_plans (tenant_id, source_session_id, created_at DESC)
    WHERE source_session_id IS NOT NULL;

ALTER TABLE dynamic_workflow_plans ENABLE ROW LEVEL SECURITY;
ALTER TABLE dynamic_workflow_plans FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_dynamic_workflow_plans ON dynamic_workflow_plans;
CREATE POLICY tenant_isolation_dynamic_workflow_plans ON dynamic_workflow_plans
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
