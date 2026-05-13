CREATE TABLE IF NOT EXISTS remote_computer_job_assignments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    execution_job_id UUID NOT NULL REFERENCES execution_jobs(id),
    remote_computer_id UUID NOT NULL REFERENCES remote_computers(id),
    lease_id UUID NOT NULL REFERENCES remote_computer_leases(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    status TEXT NOT NULL,
    assigned_by TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_computer_job_assignments_active_job
    ON remote_computer_job_assignments (tenant_id, execution_job_id)
    WHERE status = 'assigned';

CREATE INDEX IF NOT EXISTS idx_remote_computer_job_assignments_lease
    ON remote_computer_job_assignments (tenant_id, lease_id);

CREATE INDEX IF NOT EXISTS idx_remote_computer_job_assignments_session
    ON remote_computer_job_assignments (tenant_id, session_id);
