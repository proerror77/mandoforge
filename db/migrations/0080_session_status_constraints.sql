ALTER TABLE sessions
    ALTER COLUMN status SET DEFAULT 'idle',
    ADD CONSTRAINT sessions_status_check
        CHECK (status IN ('idle', 'running', 'requires_action', 'rescheduling', 'terminated', 'failed'));

ALTER TABLE session_loop_jobs
    ADD CONSTRAINT session_loop_jobs_status_check
        CHECK (status IN ('queued', 'running', 'completed', 'failed'));
