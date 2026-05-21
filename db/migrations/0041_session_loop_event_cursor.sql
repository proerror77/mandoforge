ALTER TABLE session_loop_jobs
    ADD COLUMN IF NOT EXISTS pending_event_seq_start BIGINT,
    ADD COLUMN IF NOT EXISTS pending_event_seq_end BIGINT,
    ADD COLUMN IF NOT EXISTS processed_event_seq BIGINT;

CREATE INDEX IF NOT EXISTS idx_session_loop_jobs_tenant_session_cursor
    ON session_loop_jobs (tenant_id, session_id, processed_event_seq DESC, pending_event_seq_end DESC);
