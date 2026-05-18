CREATE TABLE IF NOT EXISTS manager_agent_plans (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    manager_agent_id UUID NOT NULL REFERENCES agents(id),
    specialist_agent_id UUID REFERENCES agents(id),
    task_intake JSONB NOT NULL DEFAULT '{}',
    decomposition JSONB NOT NULL DEFAULT '{}',
    specialist_selection JSONB NOT NULL DEFAULT '{}',
    risk_classification TEXT NOT NULL,
    review JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL,
    audit_trace_id UUID REFERENCES audit_logs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- tracked tenant table: 'manager_agent_plans'

CREATE INDEX IF NOT EXISTS idx_manager_agent_plans_tenant_session_created
    ON manager_agent_plans (tenant_id, session_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_manager_agent_plans_tenant_agents
    ON manager_agent_plans (tenant_id, manager_agent_id, specialist_agent_id);

CREATE INDEX IF NOT EXISTS idx_manager_agent_plans_tenant_status
    ON manager_agent_plans (tenant_id, status);

ALTER TABLE manager_agent_plans ENABLE ROW LEVEL SECURITY;
ALTER TABLE manager_agent_plans FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_manager_agent_plans ON manager_agent_plans;
CREATE POLICY tenant_isolation_manager_agent_plans ON manager_agent_plans
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());
