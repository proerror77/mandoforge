CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_computer_job_assignments_active_session
    ON remote_computer_job_assignments (tenant_id, session_id)
    WHERE status = 'assigned';
