ALTER TABLE workflow_step_runs
    ADD COLUMN IF NOT EXISTS scheduled_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_tenant_scheduled
    ON workflow_step_runs (tenant_id, status, scheduled_at)
    WHERE scheduled_at IS NOT NULL;
