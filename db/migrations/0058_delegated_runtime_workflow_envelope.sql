ALTER TABLE workflow_definitions
    ADD COLUMN IF NOT EXISTS execution_strategy TEXT NOT NULL DEFAULT 'native_steps',
    ADD COLUMN IF NOT EXISTS runtime_adapter TEXT,
    ADD COLUMN IF NOT EXISTS runtime_mode TEXT,
    ADD COLUMN IF NOT EXISTS runtime_capability_contract JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS event_ingestion_policy TEXT NOT NULL DEFAULT 'normalized';

ALTER TABLE workflow_runs
    ADD COLUMN IF NOT EXISTS execution_strategy TEXT NOT NULL DEFAULT 'native_steps',
    ADD COLUMN IF NOT EXISTS runtime_adapter TEXT,
    ADD COLUMN IF NOT EXISTS runtime_mode TEXT,
    ADD COLUMN IF NOT EXISTS delegation_status TEXT,
    ADD COLUMN IF NOT EXISTS external_run_ref TEXT,
    ADD COLUMN IF NOT EXISTS runtime_event_cursor TEXT,
    ADD COLUMN IF NOT EXISTS runtime_envelope JSONB NOT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS idx_workflow_definitions_tenant_execution
    ON workflow_definitions (tenant_id, execution_strategy, runtime_adapter, release_state);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_tenant_runtime
    ON workflow_runs (tenant_id, execution_strategy, runtime_adapter, delegation_status, updated_at DESC);
