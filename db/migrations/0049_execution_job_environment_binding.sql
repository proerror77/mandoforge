ALTER TABLE execution_jobs
    ADD COLUMN IF NOT EXISTS environment_id UUID REFERENCES environments(id);

UPDATE execution_jobs
SET environment_id = sessions.environment_id
FROM sessions
WHERE execution_jobs.tenant_id = sessions.tenant_id
  AND execution_jobs.session_id = sessions.id
  AND execution_jobs.environment_id IS NULL
  AND sessions.environment_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_execution_jobs_environment_status
    ON execution_jobs (tenant_id, environment_id, status, enqueued_at)
    WHERE environment_id IS NOT NULL;
