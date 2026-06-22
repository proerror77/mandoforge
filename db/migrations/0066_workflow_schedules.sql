CREATE TABLE IF NOT EXISTS workflow_schedules (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    workflow_definition_id UUID REFERENCES workflow_definitions(id) ON DELETE SET NULL,
    cron_expression TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    enabled BOOLEAN NOT NULL DEFAULT true,
    next_run_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    last_run_status TEXT,
    run_args JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_workflow_schedules_tenant_enabled
    ON workflow_schedules (tenant_id, enabled, next_run_at ASC)
    WHERE enabled = true;

ALTER TABLE workflow_schedules ENABLE ROW LEVEL SECURITY;
ALTER TABLE workflow_schedules FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation_workflow_schedules ON workflow_schedules;
CREATE POLICY tenant_isolation_workflow_schedules ON workflow_schedules
    USING (tenant_id = mandoforge_current_tenant_id())
    WITH CHECK (tenant_id = mandoforge_current_tenant_id());

-- Now that the table exists, backfill the FK on workflow_runs
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_workflow_runs_source_schedule'
    ) THEN
        ALTER TABLE workflow_runs
            ADD CONSTRAINT fk_workflow_runs_source_schedule
            FOREIGN KEY (source_schedule_id) REFERENCES workflow_schedules(id) ON DELETE SET NULL;
    END IF;
END $$;
