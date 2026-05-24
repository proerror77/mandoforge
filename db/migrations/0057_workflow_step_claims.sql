ALTER TABLE workflow_step_runs
    ADD COLUMN IF NOT EXISTS claimed_by_worker TEXT,
    ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS context_packet_id UUID REFERENCES context_packets(id);

CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_tenant_agent_status
    ON workflow_step_runs (tenant_id, agent_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_tenant_claim_lease
    ON workflow_step_runs (tenant_id, status, lease_expires_at)
    WHERE claimed_by_worker IS NOT NULL;
