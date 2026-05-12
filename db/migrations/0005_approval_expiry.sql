ALTER TABLE approvals
    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_approvals_expires_at ON approvals(tenant_id, expires_at)
WHERE status = 'pending';
