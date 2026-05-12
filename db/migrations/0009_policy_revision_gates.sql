ALTER TABLE policy_revisions ADD COLUMN IF NOT EXISTS gate_status TEXT;
ALTER TABLE policy_revisions ADD COLUMN IF NOT EXISTS gate_result JSONB NOT NULL DEFAULT '{}';
ALTER TABLE policy_revisions ADD COLUMN IF NOT EXISTS gated_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_policy_revisions_gate_status ON policy_revisions(tenant_id, gate_status, created_at DESC);
